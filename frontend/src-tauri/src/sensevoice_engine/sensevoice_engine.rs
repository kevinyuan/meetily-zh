//! SenseVoice engine — fast multilingual ASR (zh / en / ja / ko / yue).
//!
//! SenseVoice is the Chinese-optimised transcription path. It is a single ~230 MB
//! int8 ONNX graph plus a tokens file, runs on the same `ort` runtime as Parakeet,
//! and does CTC decoding in one shot (no autoregressive loop), which makes it very
//! fast — roughly 60x realtime on CPU.
//!
//! Model artifacts come from sherpa-onnx's official release. The inference code
//! under `vendor/` is a vendored subset of transcribe-rs (MIT); see vendor/mod.rs
//! for why it is vendored rather than depended upon.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::vendor::onnx::sense_voice::{SenseVoiceModel, SenseVoiceParams};
use super::vendor::TranscriptionResult;
use super::vendor::onnx::Quantization;

/// Model status, mirroring the Whisper/Parakeet engines so the frontend can treat
/// all three model managers identically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelStatus {
    Available,
    Missing,
    Downloading { progress: u8 },
    Error(String),
    Corrupted { file_size: u64, expected_min_size: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub downloaded_mb: f64,
    pub total_mb: f64,
    pub speed_mbps: f64,
    pub percent: u8,
}

impl DownloadProgress {
    pub fn new(downloaded: u64, total: u64, speed_mbps: f64) -> Self {
        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };
        Self {
            downloaded_bytes: downloaded,
            total_bytes: total,
            downloaded_mb: downloaded as f64 / (1024.0 * 1024.0),
            total_mb: total as f64 / (1024.0 * 1024.0),
            speed_mbps,
            percent,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub path: PathBuf,
    pub size_mb: u32,
    pub speed: String,
    pub status: ModelStatus,
    pub description: String,
    /// Languages this model can transcribe. Drives the language filter in the UI.
    pub languages: Vec<String>,
}

/// A downloadable SenseVoice model.
struct SenseVoiceModelDef {
    name: &'static str,
    /// sherpa-onnx release tarball (.tar.bz2).
    url: &'static str,
    /// Directory the tarball expands to, inside the archive.
    archive_dir: &'static str,
    size_mb: u32,
    speed: &'static str,
    description: &'static str,
}

/// The model catalog.
///
/// Only the 2024-07-17 export is offered: it is the one that carries the ONNX
/// metadata the decoder needs (`vocab_size`, `lfr_window_size`, `neg_mean`,
/// `inv_stddev`, `lang_*`, `with_itn`) *and* supports punctuation/ITN. The newer
/// 2025-09-09 Cantonese-tuned export drops punctuation support.
const SENSEVOICE_MODELS: &[SenseVoiceModelDef] = &[SenseVoiceModelDef {
    name: "sense-voice-small-int8",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
    archive_dir: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
    size_mb: 236,
    speed: "Very Fast",
    description: "Chinese-optimised. Also English, Japanese, Korean and Cantonese. ~60x realtime on CPU, with punctuation.",
}];

/// Languages SenseVoice supports, as ISO-639-1 codes (`yue` = Cantonese).
pub const SENSEVOICE_LANGUAGES: &[&str] = &["zh", "en", "ja", "ko", "yue"];

/// One sentence, with the time it actually occupies in the audio.
#[derive(Debug, Clone)]
pub struct Sentence {
    pub text: String,
    /// Seconds from the start of this audio chunk.
    pub start: f32,
    pub end: f32,
}

/// Characters that end a sentence, in the scripts SenseVoice writes.
const SENTENCE_ENDINGS: [char; 8] = ['。', '！', '？', '.', '!', '?', '；', ';'];

/// Split a transcription into sentences using the model's own output.
///
/// The boundaries come from the punctuation SenseVoice predicts (ITN is on), and the
/// times come from its CTC token alignment — not from how long the speaker paused.
/// VAD still decides which audio goes into one inference call, but it no longer
/// decides where a transcript line ends: in fluent speech, pauses between sentences
/// are routinely shorter than the VAD's silence threshold, so whole paragraphs were
/// landing on a single line with a single timestamp.
///
/// Falls back to one sentence spanning the whole chunk when the model reports no
/// token timestamps, so a caller always gets something usable.
fn split_into_sentences(result: &TranscriptionResult, fallback_duration: f32) -> Vec<Sentence> {
    let text = result.text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let Some(tokens) = result.segments.as_ref().filter(|s| !s.is_empty()) else {
        // No alignment available: behave exactly as before (one line per VAD segment).
        return vec![Sentence {
            text: text.to_string(),
            start: 0.0,
            end: fallback_duration,
        }];
    };

    let mut sentences = Vec::new();
    let mut buffer = String::new();
    let mut sentence_start: Option<f32> = None;
    let mut sentence_end = 0.0f32;

    for token in tokens {
        let piece = token.text.replace('\u{2581}', " ");
        if piece.trim().is_empty() && buffer.is_empty() {
            continue;
        }

        if sentence_start.is_none() {
            sentence_start = Some(token.start);
        }
        sentence_end = token.end;
        buffer.push_str(&piece);

        let ends_sentence = piece
            .chars()
            .last()
            .is_some_and(|c| SENTENCE_ENDINGS.contains(&c));

        if ends_sentence {
            let trimmed = buffer.trim();
            if !trimmed.is_empty() {
                sentences.push(Sentence {
                    text: trimmed.to_string(),
                    start: sentence_start.unwrap_or(0.0),
                    end: sentence_end,
                });
            }
            buffer.clear();
            sentence_start = None;
        }
    }

    // Trailing text with no closing punctuation is still a sentence.
    let trailing = buffer.trim();
    if !trailing.is_empty() {
        sentences.push(Sentence {
            text: trailing.to_string(),
            start: sentence_start.unwrap_or(0.0),
            end: sentence_end.max(fallback_duration),
        });
    }

    if sentences.is_empty() {
        vec![Sentence {
            text: text.to_string(),
            start: 0.0,
            end: fallback_duration,
        }]
    } else {
        sentences
    }
}

/// The two files that must exist for a model directory to be usable.
const MODEL_FILE: &str = "model.int8.onnx";
const TOKENS_FILE: &str = "tokens.txt";

/// A model file smaller than this is a failed/partial download, not a model.
const MIN_MODEL_BYTES: u64 = 200 * 1024 * 1024;

pub struct SenseVoiceEngine {
    models_dir: PathBuf,
    current_model: Arc<RwLock<Option<SenseVoiceModel>>>,
    current_model_name: Arc<RwLock<Option<String>>>,
    pub(crate) available_models: Arc<RwLock<HashMap<String, ModelInfo>>>,
    cancel_download_flag: Arc<RwLock<Option<String>>>,
    pub(crate) active_downloads: Arc<RwLock<HashSet<String>>>,
}

impl SenseVoiceEngine {
    pub fn new_with_models_dir(models_dir: Option<PathBuf>) -> Result<Self> {
        let models_dir = if let Some(dir) = models_dir {
            dir.join("sensevoice")
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| anyhow!("Failed to get current directory: {}", e))?;

            if cfg!(debug_assertions) {
                current_dir.join("models").join("sensevoice")
            } else {
                dirs::data_dir()
                    .or_else(dirs::home_dir)
                    .ok_or_else(|| anyhow!("Could not find system data directory"))?
                    .join("Meetily")
                    .join("models")
                    .join("sensevoice")
            }
        };

        log::info!(
            "SenseVoiceEngine using models directory: {}",
            models_dir.display()
        );

        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir)?;
        }

        Ok(Self {
            models_dir,
            current_model: Arc::new(RwLock::new(None)),
            current_model_name: Arc::new(RwLock::new(None)),
            available_models: Arc::new(RwLock::new(HashMap::new())),
            cancel_download_flag: Arc::new(RwLock::new(None)),
            active_downloads: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Directory a given model lives in once installed.
    fn model_dir(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name)
    }

    /// A model is present iff both the graph and the tokens file are there and the
    /// graph is not a truncated download.
    fn inspect_model_dir(dir: &PathBuf) -> ModelStatus {
        let model_path = dir.join(MODEL_FILE);
        let tokens_path = dir.join(TOKENS_FILE);

        if !model_path.exists() || !tokens_path.exists() {
            return ModelStatus::Missing;
        }

        match std::fs::metadata(&model_path) {
            Ok(meta) if meta.len() < MIN_MODEL_BYTES => ModelStatus::Corrupted {
                file_size: meta.len(),
                expected_min_size: MIN_MODEL_BYTES,
            },
            Ok(_) => ModelStatus::Available,
            Err(e) => ModelStatus::Error(e.to_string()),
        }
    }

    pub async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let active = self.active_downloads.read().await.clone();
        let mut models = Vec::new();

        for def in SENSEVOICE_MODELS {
            let dir = self.model_dir(def.name);

            let status = if active.contains(def.name) {
                ModelStatus::Downloading { progress: 0 }
            } else {
                Self::inspect_model_dir(&dir)
            };

            models.push(ModelInfo {
                name: def.name.to_string(),
                path: dir,
                size_mb: def.size_mb,
                speed: def.speed.to_string(),
                status,
                description: def.description.to_string(),
                languages: SENSEVOICE_LANGUAGES.iter().map(|s| s.to_string()).collect(),
            });
        }

        let mut cache = self.available_models.write().await;
        cache.clear();
        for model in &models {
            cache.insert(model.name.clone(), model.clone());
        }

        Ok(models)
    }

    pub async fn load_model(&self, model_name: &str) -> Result<()> {
        let dir = self.model_dir(model_name);

        if !matches!(Self::inspect_model_dir(&dir), ModelStatus::Available) {
            return Err(anyhow!(
                "SenseVoice model '{}' is not installed. Download it first.",
                model_name
            ));
        }

        // Already loaded — nothing to do.
        if self.current_model_name.read().await.as_deref() == Some(model_name) {
            return Ok(());
        }

        log::info!("Loading SenseVoice model from {}", dir.display());
        let started = std::time::Instant::now();

        // ONNX session creation is blocking and takes ~350ms; keep it off the async
        // runtime's worker threads.
        let dir_for_load = dir.clone();
        let model = tokio::task::spawn_blocking(move || {
            SenseVoiceModel::load(&dir_for_load, &Quantization::Int8)
        })
        .await
        .map_err(|e| anyhow!("SenseVoice load task panicked: {}", e))?
        .map_err(|e| anyhow!("Failed to load SenseVoice model: {}", e))?;

        *self.current_model.write().await = Some(model);
        *self.current_model_name.write().await = Some(model_name.to_string());

        log::info!(
            "SenseVoice model '{}' loaded in {:?}",
            model_name,
            started.elapsed()
        );
        Ok(())
    }

    pub async fn unload_model(&self) -> bool {
        let had_model = self.current_model.write().await.take().is_some();
        *self.current_model_name.write().await = None;
        had_model
    }

    pub async fn get_current_model(&self) -> Option<String> {
        self.current_model_name.read().await.clone()
    }

    pub async fn is_model_loaded(&self) -> bool {
        self.current_model.read().await.is_some()
    }

    /// Transcribe 16 kHz mono f32 samples in [-1, 1].
    ///
    /// Returns the utterance split into sentences (see `split_into_sentences`), plus
    /// the language SenseVoice *detected* for it.
    ///
    /// `language` is a hint; `None` (or an unsupported code) lets SenseVoice run its
    /// own language identification. Because the pipeline hands us one VAD-segmented
    /// chunk at a time, leaving the hint as `None` gives per-sentence detection —
    /// which is what makes a meeting that switches between Chinese and English
    /// transcribe correctly instead of forcing every sentence through one language.
    pub async fn transcribe_audio(
        &self,
        audio_data: Vec<f32>,
        language: Option<String>,
    ) -> Result<(Vec<Sentence>, Option<String>)> {
        let mut guard = self.current_model.write().await;
        let model = guard
            .as_mut()
            .ok_or_else(|| anyhow!("No SenseVoice model loaded. Please load a model first."))?;

        let language = language.and_then(|l| normalize_language(&l));

        log::debug!(
            "SenseVoice transcribing {} samples ({:.1}s), language={:?}",
            audio_data.len(),
            audio_data.len() as f64 / 16_000.0,
            language,
        );

        let params = SenseVoiceParams {
            language,
            // Inverse text normalisation: renders "五十" as "50" and restores
            // punctuation. This export supports it; the 2025-09-09 one does not.
            use_itn: Some(true),
        };

        let chunk_duration = audio_data.len() as f32 / 16_000.0;

        let result = model
            .transcribe_with(&audio_data, &params)
            .map_err(|e| anyhow!("SenseVoice transcription failed: {}", e))?;

        if let Some(detected) = &result.language {
            log::debug!("SenseVoice detected language for this utterance: {}", detected);
        }

        let sentences = split_into_sentences(&result, chunk_duration);
        log::debug!(
            "SenseVoice produced {} sentence(s) from a {:.1}s segment",
            sentences.len(),
            chunk_duration
        );

        Ok((sentences, result.language))
    }

    pub async fn get_models_directory(&self) -> PathBuf {
        self.models_dir.clone()
    }

    pub async fn delete_model(&self, model_name: &str) -> Result<String> {
        let dir = self.model_dir(model_name);
        if !dir.exists() {
            return Err(anyhow!("Model '{}' is not installed", model_name));
        }

        // Don't delete the model we're currently running.
        if self.current_model_name.read().await.as_deref() == Some(model_name) {
            self.unload_model().await;
        }

        std::fs::remove_dir_all(&dir)?;
        self.discover_models().await?;

        Ok(format!("Deleted SenseVoice model '{}'", model_name))
    }

    pub async fn cancel_download(&self, model_name: &str) -> Result<()> {
        *self.cancel_download_flag.write().await = Some(model_name.to_string());
        Ok(())
    }

    /// Download and install a model, reporting progress via `on_progress`.
    ///
    /// The archive is a `.tar.bz2` (sherpa-onnx's format), streamed to a temp file,
    /// then extracted. Extraction pulls only the two files we need out of the
    /// archive's nested directory, so the installed layout is flat:
    /// `models/sensevoice/<model-name>/{model.int8.onnx,tokens.txt}`.
    pub async fn download_model<F>(&self, model_name: &str, on_progress: F) -> Result<()>
    where
        F: Fn(DownloadProgress) + Send + 'static,
    {
        let def = SENSEVOICE_MODELS
            .iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| anyhow!("Unknown SenseVoice model: {}", model_name))?;

        // Guard against two concurrent downloads of the same model.
        {
            let mut active = self.active_downloads.write().await;
            if !active.insert(model_name.to_string()) {
                return Err(anyhow!("Model '{}' is already downloading", model_name));
            }
        }

        let result = self.download_model_inner(def, on_progress).await;

        self.active_downloads.write().await.remove(model_name);
        if self.cancel_download_flag.read().await.as_deref() == Some(model_name) {
            *self.cancel_download_flag.write().await = None;
        }

        result
    }

    async fn download_model_inner<F>(&self, def: &SenseVoiceModelDef, on_progress: F) -> Result<()>
    where
        F: Fn(DownloadProgress) + Send + 'static,
    {
        use futures_util::StreamExt;
        use std::io::Write;

        let target_dir = self.model_dir(def.name);
        let tmp_archive = self.models_dir.join(format!("{}.tar.bz2.partial", def.name));

        log::info!("Downloading SenseVoice model from {}", def.url);

        let response = reqwest::get(def.url)
            .await
            .map_err(|e| anyhow!("Failed to start download: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow!("Download failed: {}", e))?;

        let total_bytes = response.content_length().unwrap_or(0);
        let mut file = std::fs::File::create(&tmp_archive)?;
        let mut stream = response.bytes_stream();

        let mut downloaded: u64 = 0;
        let started = std::time::Instant::now();
        let mut last_report = std::time::Instant::now();
        let mut last_percent = 0u8;

        while let Some(chunk) = stream.next().await {
            // Cancellation: drop the partial file and bail.
            if self.cancel_download_flag.read().await.as_deref() == Some(def.name) {
                drop(file);
                let _ = std::fs::remove_file(&tmp_archive);
                log::info!("SenseVoice download cancelled: {}", def.name);
                return Err(anyhow!("Download cancelled"));
            }

            let chunk = chunk.map_err(|e| anyhow!("Download stream error: {}", e))?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            // Throttle progress events: at most one per percent, and at most one
            // every 250ms, so we don't flood the webview.
            let progress = DownloadProgress::new(
                downloaded,
                total_bytes,
                downloaded as f64 / (1024.0 * 1024.0) / started.elapsed().as_secs_f64().max(0.001),
            );
            if progress.percent != last_percent
                && last_report.elapsed() >= std::time::Duration::from_millis(250)
            {
                last_percent = progress.percent;
                last_report = std::time::Instant::now();
                on_progress(progress);
            }
        }

        file.flush()?;
        drop(file);

        log::info!(
            "SenseVoice archive downloaded ({} MB), extracting...",
            downloaded / (1024 * 1024)
        );

        let archive_dir = def.archive_dir.to_string();
        let tmp_for_extract = tmp_archive.clone();
        let target_for_extract = target_dir.clone();

        // Decompression is CPU-bound and synchronous.
        tokio::task::spawn_blocking(move || {
            extract_model(&tmp_for_extract, &archive_dir, &target_for_extract)
        })
        .await
        .map_err(|e| anyhow!("Extraction task panicked: {}", e))??;

        let _ = std::fs::remove_file(&tmp_archive);

        // Report 100% only once the model is actually usable on disk.
        on_progress(DownloadProgress::new(total_bytes.max(downloaded), total_bytes.max(downloaded), 0.0));

        match Self::inspect_model_dir(&target_dir) {
            ModelStatus::Available => {
                self.discover_models().await?;
                log::info!("SenseVoice model '{}' installed", def.name);
                Ok(())
            }
            other => Err(anyhow!(
                "SenseVoice model '{}' is unusable after extraction: {:?}",
                def.name,
                other
            )),
        }
    }
}

/// Pull `model.int8.onnx` and `tokens.txt` out of the sherpa tarball into `target`.
///
/// The archive nests everything under a versioned directory; we flatten it so the
/// on-disk layout doesn't encode the sherpa release name.
fn extract_model(archive: &PathBuf, archive_dir: &str, target: &PathBuf) -> Result<()> {
    use bzip2::read::BzDecoder;
    use std::io::copy;

    // Extract into a staging dir, then swap into place, so a crash mid-extract can
    // never leave a half-populated model directory that looks Available.
    let staging = target.with_extension("extracting");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;

    let file = std::fs::File::open(archive)?;
    let mut tar = tar::Archive::new(BzDecoder::new(file));

    let mut found = 0;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name != MODEL_FILE && name != TOKENS_FILE {
            continue;
        }
        // Guard against path traversal and against picking up a same-named file
        // from an unexpected place in the archive.
        if !path.starts_with(archive_dir) {
            continue;
        }

        let mut out = std::fs::File::create(staging.join(name))?;
        copy(&mut entry, &mut out)?;
        found += 1;
    }

    if found != 2 {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(anyhow!(
            "Archive did not contain both {} and {} (found {})",
            MODEL_FILE,
            TOKENS_FILE,
            found
        ));
    }

    if target.exists() {
        std::fs::remove_dir_all(target)?;
    }
    std::fs::rename(&staging, target)?;

    Ok(())
}

/// Map a UI/BCP-47 language tag onto a code SenseVoice understands.
///
/// Chinese scripts collapse to `zh` — SenseVoice has no script switch, and the
/// engine would reject `zh-Hans`. Anything it can't handle returns `None`, which
/// means "let the model auto-detect" rather than "fail".
pub fn normalize_language(language: &str) -> Option<String> {
    let lower = language.to_lowercase();

    if lower == "auto" || lower == "auto-translate" {
        return None;
    }

    let base = lower.split(['-', '_']).next().unwrap_or(&lower);

    // Cantonese arrives as `yue`, but also as `zh-yue` / `zh-HK`.
    if lower.starts_with("yue") || lower.contains("yue") || lower.contains("hk") {
        return Some("yue".to_string());
    }

    if SENSEVOICE_LANGUAGES.contains(&base) {
        return Some(base.to_string());
    }

    None
}

#[cfg(test)]
mod sentence_tests {
    use super::*;
    use crate::sensevoice_engine::vendor::TranscriptionSegment;

    fn tok(text: &str, start: f32, end: f32) -> TranscriptionSegment {
        TranscriptionSegment { start, end, text: text.to_string() }
    }

    fn result(text: &str, tokens: Vec<TranscriptionSegment>) -> TranscriptionResult {
        TranscriptionResult {
            text: text.to_string(),
            segments: Some(tokens),
            language: Some("zh".to_string()),
        }
    }

    /// The point of the whole exercise: one VAD segment holding several sentences must
    /// become several transcript lines, each with its own real timestamps.
    #[test]
    fn splits_on_punctuation_with_aligned_times() {
        let r = result(
            "今天开会。明天休息。",
            vec![
                tok("今天", 0.0, 0.3),
                tok("开会", 0.3, 0.6),
                tok("。", 0.6, 0.66),
                tok("明天", 0.9, 1.2),
                tok("休息", 1.2, 1.5),
                tok("。", 1.5, 1.56),
            ],
        );

        let s = split_into_sentences(&r, 2.0);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "今天开会。");
        assert_eq!(s[1].text, "明天休息。");
        // Times come from the token alignment, not from the chunk boundaries.
        assert!((s[0].start - 0.0).abs() < 1e-6);
        assert!((s[0].end - 0.66).abs() < 1e-6);
        assert!((s[1].start - 0.9).abs() < 1e-6);
        assert!((s[1].end - 1.56).abs() < 1e-6);
    }

    #[test]
    fn handles_english_punctuation() {
        let r = result(
            "Hello there. How are you?",
            vec![
                tok("Hello", 0.0, 0.3),
                tok(" there", 0.3, 0.6),
                tok(".", 0.6, 0.66),
                tok(" How", 0.8, 1.0),
                tok(" are", 1.0, 1.2),
                tok(" you", 1.2, 1.4),
                tok("?", 1.4, 1.46),
            ],
        );
        let s = split_into_sentences(&r, 2.0);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "Hello there.");
        assert_eq!(s[1].text, "How are you?");
    }

    /// Trailing speech with no closing punctuation must not be dropped.
    #[test]
    fn keeps_a_trailing_fragment() {
        let r = result(
            "好的。然后",
            vec![
                tok("好的", 0.0, 0.3),
                tok("。", 0.3, 0.36),
                tok("然后", 0.5, 0.8),
            ],
        );
        let s = split_into_sentences(&r, 1.0);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].text, "然后");
    }

    /// Without alignment we must degrade to the old behaviour, not lose the text.
    #[test]
    fn falls_back_to_one_line_without_token_times() {
        let r = TranscriptionResult {
            text: "没有时间戳的一段话".to_string(),
            segments: None,
            language: None,
        };
        let s = split_into_sentences(&r, 3.5);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "没有时间戳的一段话");
        assert!((s[0].end - 3.5).abs() < 1e-6);
    }

    #[test]
    fn empty_transcription_yields_no_sentences() {
        let r = TranscriptionResult { text: "   ".to_string(), segments: None, language: None };
        assert!(split_into_sentences(&r, 1.0).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_chinese_scripts_to_zh() {
        assert_eq!(normalize_language("zh"), Some("zh".into()));
        assert_eq!(normalize_language("zh-Hans"), Some("zh".into()));
        assert_eq!(normalize_language("zh-Hant"), Some("zh".into()));
        assert_eq!(normalize_language("zh-CN"), Some("zh".into()));
    }

    #[test]
    fn maps_cantonese_variants() {
        assert_eq!(normalize_language("yue"), Some("yue".into()));
        assert_eq!(normalize_language("zh-HK"), Some("yue".into()));
    }

    #[test]
    fn auto_and_unsupported_fall_back_to_model_lid() {
        assert_eq!(normalize_language("auto"), None);
        assert_eq!(normalize_language("auto-translate"), None);
        // French is not a SenseVoice language: auto-detect rather than error.
        assert_eq!(normalize_language("fr"), None);
    }

    #[test]
    fn supported_languages_pass_through() {
        for lang in ["en", "ja", "ko"] {
            assert_eq!(normalize_language(lang), Some(lang.to_string()));
        }
    }
}

#[cfg(test)]
mod extract_tests {
    use super::*;

    /// Exercises the real sherpa-onnx tarball end-to-end: bz2 decode, selective
    /// extraction of the two files we need, flattening of the archive's nested
    /// directory, and the staging -> atomic-rename swap.
    ///
    /// Ignored by default (needs a 236 MB artifact). Run with:
    ///   SENSEVOICE_TARBALL=/path/to.tar.bz2 cargo test extract_real_tarball -- --ignored --nocapture
    #[test]
    #[ignore]
    fn extract_real_tarball() {
        let archive = std::path::PathBuf::from(
            std::env::var("SENSEVOICE_TARBALL").expect("set SENSEVOICE_TARBALL"),
        );
        let tmp = std::env::temp_dir().join("sv-extract-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let target = tmp.join("sense-voice-small-int8");

        extract_model(
            &archive,
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            &target,
        )
        .expect("extraction failed");

        assert!(target.join(MODEL_FILE).exists(), "model.int8.onnx missing");
        assert!(target.join(TOKENS_FILE).exists(), "tokens.txt missing");
        assert!(
            !target.with_extension("extracting").exists(),
            "staging dir was not swapped away"
        );
        assert!(
            matches!(
                SenseVoiceEngine::inspect_model_dir(&target),
                ModelStatus::Available
            ),
            "extracted model does not report Available"
        );

        let size = std::fs::metadata(target.join(MODEL_FILE)).unwrap().len();
        println!("extracted model.int8.onnx = {} MB", size / (1024 * 1024));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
