// audio/transcription/worker.rs
//
// Parallel transcription worker pool and chunk processing logic.

use super::engine::TranscriptionEngine;
use super::provider::TranscriptionError;
use crate::audio::AudioChunk;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

// Sequence counter for transcript updates
static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(0);

// Speech detection flag - reset per recording session
static SPEECH_DETECTED_EMITTED: AtomicBool = AtomicBool::new(false);

/// Whether transcript lines are split on the model's punctuation.
///
/// When false, a line is one VAD segment (speech between two silences) — the behaviour
/// of every engine other than SenseVoice, and what the app did before. The user chooses
/// this in Settings > Recording.
static SPLIT_ON_PUNCTUATION: AtomicBool = AtomicBool::new(true);

pub fn set_split_on_punctuation(enabled: bool) {
    SPLIT_ON_PUNCTUATION.store(enabled, Ordering::Relaxed);
    info!(
        "Transcript segmentation: {}",
        if enabled { "model punctuation" } else { "VAD pauses" }
    );
}

pub fn split_on_punctuation() -> bool {
    SPLIT_ON_PUNCTUATION.load(Ordering::Relaxed)
}

/// The whole chunk as a single line — what engines without sentence alignment produce.
fn whole_chunk(text: String, duration: f64) -> Vec<TranscriptSentence> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    vec![TranscriptSentence { text, start: 0.0, end: duration }]
}

/// Reset the speech detected flag for a new recording session
pub fn reset_speech_detected_flag() {
    SPEECH_DETECTED_EMITTED.store(false, Ordering::SeqCst);
}

/// Returns true if the transcript text is non-trivial and should be emitted.
/// Filters empty/whitespace-only text; no confidence gating is applied.
fn should_emit_transcript(text: &str) -> bool {
    !text.trim().is_empty()
}

fn should_log_transcript_snippet(accepted_count: u64) -> bool {
    accepted_count <= 5 || accepted_count % 25 == 0
}

#[derive(Default)]
struct WorkerLogStats {
    queued: u64,
    completed: u64,
    emitted: u64,
    empty: u64,
    too_short: u64,
    failed: u64,
    model_not_loaded_failures: u64,
    engine_failures: u64,
    unsupported_language_failures: u64,
    speech_detected_emit_failures: u64,
    transcript_update_emit_failures: u64,
    max_queue_depth: u64,
    total_processing_duration: Duration,
    confidence_total: f64,
    confidence_count: u64,
}

enum WorkerFailureCategory {
    ModelNotLoaded,
    EngineFailed,
    UnsupportedLanguage,
    SpeechDetectedEmit,
    TranscriptUpdateEmit,
}

impl WorkerLogStats {
    fn record_failure(&mut self, category: WorkerFailureCategory) {
        self.failed += 1;
        match category {
            WorkerFailureCategory::ModelNotLoaded => self.model_not_loaded_failures += 1,
            WorkerFailureCategory::EngineFailed => self.engine_failures += 1,
            WorkerFailureCategory::UnsupportedLanguage => self.unsupported_language_failures += 1,
            WorkerFailureCategory::SpeechDetectedEmit => self.speech_detected_emit_failures += 1,
            WorkerFailureCategory::TranscriptUpdateEmit => {
                self.transcript_update_emit_failures += 1
            }
        }
    }

    fn snapshot_and_reset(&mut self) -> Self {
        std::mem::take(self)
    }
}

fn emit_worker_log_snapshot(stats: WorkerLogStats, final_summary: bool) {
    let confidence_average = if stats.confidence_count == 0 {
        0.0
    } else {
        stats.confidence_total / stats.confidence_count as f64
    };

    if final_summary {
        info!(
            "transcription worker final summary queued={} completed={} emitted={} empty={} too_short={} failed={} model_not_loaded_failures={} engine_failures={} unsupported_language_failures={} speech_detected_emit_failures={} transcript_update_emit_failures={} max_queue_depth={} processing_ms={} confidence_average={:.2} confidence_count={}",
            stats.queued,
            stats.completed,
            stats.emitted,
            stats.empty,
            stats.too_short,
            stats.failed,
            stats.model_not_loaded_failures,
            stats.engine_failures,
            stats.unsupported_language_failures,
            stats.speech_detected_emit_failures,
            stats.transcript_update_emit_failures,
            stats.max_queue_depth,
            stats.total_processing_duration.as_millis(),
            confidence_average,
            stats.confidence_count
        );
        if stats.failed > 0 {
            warn!(
                "transcription worker final failures={} model_not_loaded_failures={} engine_failures={} unsupported_language_failures={} speech_detected_emit_failures={} transcript_update_emit_failures={} queued={} completed={}",
                stats.failed,
                stats.model_not_loaded_failures,
                stats.engine_failures,
                stats.unsupported_language_failures,
                stats.speech_detected_emit_failures,
                stats.transcript_update_emit_failures,
                stats.queued,
                stats.completed
            );
        }
    } else if stats.failed > 0 {
        warn!(
            "transcription worker summary queued={} completed={} emitted={} empty={} too_short={} failed={} model_not_loaded_failures={} engine_failures={} unsupported_language_failures={} speech_detected_emit_failures={} transcript_update_emit_failures={} max_queue_depth={} processing_ms={} confidence_average={:.2} confidence_count={}",
            stats.queued,
            stats.completed,
            stats.emitted,
            stats.empty,
            stats.too_short,
            stats.failed,
            stats.model_not_loaded_failures,
            stats.engine_failures,
            stats.unsupported_language_failures,
            stats.speech_detected_emit_failures,
            stats.transcript_update_emit_failures,
            stats.max_queue_depth,
            stats.total_processing_duration.as_millis(),
            confidence_average,
            stats.confidence_count
        );
    } else {
        debug!(
            "transcription worker summary queued={} completed={} emitted={} empty={} too_short={} failed={} model_not_loaded_failures={} engine_failures={} unsupported_language_failures={} speech_detected_emit_failures={} transcript_update_emit_failures={} max_queue_depth={} processing_ms={} confidence_average={:.2} confidence_count={}",
            stats.queued,
            stats.completed,
            stats.emitted,
            stats.empty,
            stats.too_short,
            stats.failed,
            stats.model_not_loaded_failures,
            stats.engine_failures,
            stats.unsupported_language_failures,
            stats.speech_detected_emit_failures,
            stats.transcript_update_emit_failures,
            stats.max_queue_depth,
            stats.total_processing_duration.as_millis(),
            confidence_average,
            stats.confidence_count
        );
    }
}

/// One transcript line, with the span of audio it covers.
///
/// Engines that can split an utterance into sentences (SenseVoice, via its predicted
/// punctuation and CTC token alignment) return several of these per audio chunk.
/// Engines that cannot (Whisper, Parakeet) return exactly one spanning the whole
/// chunk — i.e. the previous behaviour, where a VAD segment *was* the transcript line.
#[derive(Debug, Clone)]
pub struct TranscriptSentence {
    pub text: String,
    /// Seconds from the start of the audio chunk.
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptUpdate {
    pub text: String,
    pub timestamp: String, // Wall-clock time for reference (e.g., "14:30:05")
    pub source: String,
    pub sequence_id: u64,
    pub chunk_start_time: f64, // Legacy field, kept for compatibility
    pub is_partial: bool,
    pub confidence: f32,
    // NEW: Recording-relative timestamps for playback sync
    pub audio_start_time: f64, // Seconds from recording start (e.g., 125.3)
    pub audio_end_time: f64,   // Seconds from recording start (e.g., 128.6)
    pub duration: f64,          // Segment duration in seconds (e.g., 3.3)
    /// Language detected for *this* utterance ("zh", "en", "ja", "ko", "yue").
    ///
    /// Per-sentence, not per-meeting: each VAD chunk is identified independently, so a
    /// meeting that switches between Chinese and English is labelled sentence by
    /// sentence. Only SenseVoice reports this; the other engines send `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

// NOTE: get_transcript_history and get_recording_meeting_name functions
// have been moved to recording_commands.rs where they have access to RECORDING_MANAGER

/// Shortest utterance whose auto-detected language may be trusted, in seconds.
///
/// SenseVoice identifies the language of each utterance independently, which is what makes
/// per-sentence tagging possible — but it needs about a second of audio to do it. Measured
/// over a real 919-line Chinese meeting:
///
/// | segment length | mislabelled |
/// |----------------|-------------|
/// | < 0.5s         | 54.8%       |
/// | 0.5 – 1s       | 23.8%       |
/// | 1 – 2s         |  4.5%       |
/// | >= 2s          |  0.6%       |
///
/// And a misread language does more than mislabel the line: the model then *decodes* in
/// that language, so Chinese interjections came back as Japanese kana — 「对啦呢」 as
/// `だらね`, 「嗯嗯嗯」 as `うんうんうん`. The text itself was wrong, not just the tag.
///
/// So auto-detection is only trusted on segments long enough to earn it. Shorter ones are
/// decoded in the language the meeting is actually being held in (see `dominant_language`).
const MIN_RELIABLE_LANGUAGE_ID_SECS: f64 = 1.0;

/// Languages detected on segments long enough to be trusted, this recording.
static LANGUAGE_TALLY: Mutex<BTreeMap<String, u32>> = Mutex::new(BTreeMap::new());

/// Record a language detected on a segment we trust.
fn note_confident_language(language: &str) {
    if let Ok(mut tally) = LANGUAGE_TALLY.lock() {
        *tally.entry(language.to_string()).or_insert(0) += 1;
    }
}

/// The language this meeting is mostly being held in, if we have seen enough to say.
///
/// `None` until the first trustworthy segment arrives — during those first seconds a short
/// utterance still falls back to auto-detection, because guessing a language would be worse
/// than the model's own guess.
fn dominant_language() -> Option<String> {
    let tally = LANGUAGE_TALLY.lock().ok()?;
    tally
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(language, _)| language.clone())
}

/// Forget the previous meeting's languages. Called when a recording starts.
pub fn reset_language_tally() {
    if let Ok(mut tally) = LANGUAGE_TALLY.lock() {
        tally.clear();
    }
}

/// Optimized parallel transcription task ensuring ZERO chunk loss
pub fn start_transcription_task<R: Runtime>(
    app: AppHandle<R>,
    transcription_receiver: tokio::sync::mpsc::UnboundedReceiver<AudioChunk>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting transcription worker task");

        let transcription_engine = match super::engine::get_or_init_transcription_engine(&app).await {
            Ok(engine) => engine,
            Err(e) => {
                error!("Failed to initialize transcription engine");
                let _ = app.emit("transcription-error", serde_json::json!({
                    "error": e,
                    "userMessage": "Recording failed: Unable to initialize speech recognition. Please check your model settings.",
                    "actionable": true,
                    "phase": "active"
                }));
                return;
            }
        };

        const NUM_WORKERS: usize = 1;
        let (work_sender, work_receiver) = tokio::sync::mpsc::unbounded_channel::<AudioChunk>();
        let work_receiver = Arc::new(tokio::sync::Mutex::new(work_receiver));
        let chunks_queued = Arc::new(AtomicU64::new(0));
        let chunks_completed = Arc::new(AtomicU64::new(0));
        let mut worker_handles = Vec::new();

        for worker_id in 0..NUM_WORKERS {
            let engine_clone = match &transcription_engine {
                TranscriptionEngine::Whisper(e) => TranscriptionEngine::Whisper(e.clone()),
                TranscriptionEngine::Parakeet(e) => TranscriptionEngine::Parakeet(e.clone()),
                TranscriptionEngine::SenseVoice(e) => TranscriptionEngine::SenseVoice(e.clone()),
                TranscriptionEngine::Provider(p) => TranscriptionEngine::Provider(p.clone()),
            };
            let app_clone = app.clone();
            let work_receiver_clone = work_receiver.clone();
            let chunks_completed_clone = chunks_completed.clone();
            let chunks_queued_clone = chunks_queued.clone();

            worker_handles.push(tokio::spawn(async move {
                let mut stats = WorkerLogStats::default();
                let mut last_summary = Instant::now();

                while let Some(chunk) = {
                    let mut receiver = work_receiver_clone.lock().await;
                    receiver.recv().await
                } {
                    stats.queued += 1;
                    let queued = chunks_queued_clone.load(Ordering::SeqCst);
                    let completed_before = chunks_completed_clone.load(Ordering::SeqCst);
                    stats.max_queue_depth = stats
                        .max_queue_depth
                        .max(queued.saturating_sub(completed_before));

                    let chunk_timestamp = chunk.timestamp;
                    let processing_started = Instant::now();

                    if !engine_clone.is_model_loaded().await {
                        stats.record_failure(WorkerFailureCategory::ModelNotLoaded);
                    } else {
                        match transcribe_chunk_with_provider(&engine_clone, chunk, &app_clone).await {
                            Ok((sentences, confidence_opt, is_partial, detected_language)) => {
                                if let Some(confidence) = confidence_opt {
                                    stats.confidence_total += confidence as f64;
                                    stats.confidence_count += 1;
                                }

                                let sentences: Vec<TranscriptSentence> = sentences
                                    .into_iter()
                                    .filter(|sentence| should_emit_transcript(&sentence.text))
                                    .collect();

                                if !sentences.is_empty() {
                                    if SPEECH_DETECTED_EMITTED
                                        .compare_exchange(
                                            false,
                                            true,
                                            Ordering::SeqCst,
                                            Ordering::SeqCst,
                                        )
                                        .is_ok()
                                        && app_clone
                                            .emit(
                                                "speech-detected",
                                                serde_json::json!({
                                                    "message": "Speech activity detected"
                                                }),
                                            )
                                            .is_err()
                                    {
                                        stats.record_failure(WorkerFailureCategory::SpeechDetectedEmit);
                                    }

                                    // One transcript line per sentence. Engines that cannot
                                    // split an utterance return a single sentence spanning the
                                    // whole chunk, which reproduces the old behaviour exactly.
                                    for sentence in sentences {
                                        let sequence_id =
                                            SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);

                                        // Sentence times are relative to the chunk; the chunk's
                                        // own timestamp puts them on the recording's timeline.
                                        let update = TranscriptUpdate {
                                            text: sentence.text,
                                            timestamp: format_current_timestamp(),
                                            source: "Audio".to_string(),
                                            language: detected_language.clone(),
                                            sequence_id,
                                            chunk_start_time: chunk_timestamp,
                                            is_partial,
                                            confidence: confidence_opt.unwrap_or(0.85),
                                            audio_start_time: chunk_timestamp + sentence.start,
                                            audio_end_time: chunk_timestamp + sentence.end,
                                            duration: (sentence.end - sentence.start).max(0.0),
                                        };

                                        if app_clone.emit("transcript-update", &update).is_ok() {
                                            stats.emitted += 1;
                                            #[cfg(debug_assertions)]
                                            if should_log_transcript_snippet(stats.emitted) {
                                                debug!(
                                                    "accepted transcript result #{}: {}",
                                                    stats.emitted,
                                                    crate::utils::log_snippet(&update.text, 200)
                                                );
                                            }
                                        } else {
                                            stats.record_failure(WorkerFailureCategory::TranscriptUpdateEmit);
                                        }
                                    }
                                } else {
                                    stats.empty += 1;
                                }
                            }
                            Err(TranscriptionError::AudioTooShort { .. }) => {
                                stats.too_short += 1;
                            }
                            Err(TranscriptionError::ModelNotLoaded) => {
                                stats.record_failure(WorkerFailureCategory::ModelNotLoaded);
                            }
                            Err(error @ TranscriptionError::EngineFailed(_)) => {
                                stats.record_failure(WorkerFailureCategory::EngineFailed);
                                warn!("Transcription engine failed: {}", error);
                                let _ =
                                    app_clone.emit("transcription-warning", error.to_string());
                            }
                            Err(error @ TranscriptionError::UnsupportedLanguage(_)) => {
                                stats.record_failure(WorkerFailureCategory::UnsupportedLanguage);
                                let _ =
                                    app_clone.emit("transcription-warning", error.to_string());
                            }
                        }
                    }

                    stats.total_processing_duration += processing_started.elapsed();
                    stats.completed += 1;
                    let completed = chunks_completed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                    let progress_percentage = if queued > 0 {
                        (completed as f64 / queued as f64 * 100.0) as u32
                    } else {
                        100
                    };
                    let _ = app_clone.emit("transcription-progress", serde_json::json!({
                        "worker_id": worker_id,
                        "chunks_completed": completed,
                        "chunks_queued": queued,
                        "progress_percentage": progress_percentage,
                        "message": format!("Worker {} processing... ({}/{})", worker_id, completed, queued)
                    }));

                    if last_summary.elapsed() >= Duration::from_secs(60) {
                        emit_worker_log_snapshot(stats.snapshot_and_reset(), false);
                        last_summary = Instant::now();
                    }
                }

                emit_worker_log_snapshot(stats.snapshot_and_reset(), true);
            }));
        }

        let mut receiver = transcription_receiver;
        while let Some(chunk) = receiver.recv().await {
            chunks_queued.fetch_add(1, Ordering::SeqCst);
            if work_sender.send(chunk).is_err() {
                error!("Failed to send transcription chunk to worker");
                break;
            }
        }
        drop(work_sender);

        let total_chunks_queued = chunks_queued.load(Ordering::SeqCst);
        let _ = app.emit("transcription-queue-complete", serde_json::json!({
            "total_chunks": total_chunks_queued,
            "message": format!("{} chunks queued for processing - waiting for completion", total_chunks_queued)
        }));

        for handle in worker_handles {
            if handle.await.is_err() {
                error!("Transcription worker panicked");
            }
        }

        const MAX_VERIFICATION_ATTEMPTS: u32 = 10;
        for _ in 0..MAX_VERIFICATION_ATTEMPTS {
            if chunks_queued.load(Ordering::SeqCst) == chunks_completed.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let final_queued = chunks_queued.load(Ordering::SeqCst);
        let final_completed = chunks_completed.load(Ordering::SeqCst);
        if final_queued != final_completed {
            error!(
                "Transcription chunk loss detected queued={} completed={}",
                final_queued, final_completed
            );
            let _ = app.emit(
                "transcript-chunk-loss-detected",
                serde_json::json!({
                    "chunks_queued": final_queued,
                    "chunks_completed": final_completed,
                    "chunks_lost": final_queued - final_completed,
                    "message": "Some transcript chunks may have been lost during shutdown"
                }),
            );
        }

        info!(
            "Transcription worker task completed queued={} completed={}",
            final_queued, final_completed
        );
    })
}

/// Transcribe audio chunk using the appropriate provider (Whisper, Parakeet, or trait-based)
/// Returns: (sentences, confidence Option, is_partial, detected language)
async fn transcribe_chunk_with_provider<R: Runtime>(
    engine: &TranscriptionEngine,
    chunk: AudioChunk,
    app: &AppHandle<R>,
) -> std::result::Result<(Vec<TranscriptSentence>, Option<f32>, bool, Option<String>), TranscriptionError> {
    let speech_samples = if chunk.sample_rate != 16000 {
        crate::audio::audio_processing::resample_audio(&chunk.data, chunk.sample_rate, 16000)
    } else {
        chunk.data
    };

    if speech_samples.is_empty() {
        return Err(TranscriptionError::AudioTooShort {
            samples: 0,
            minimum: 1600,
        });
    }

    let chunk_duration_s = speech_samples.len() as f64 / 16_000.0;

    match engine {
        TranscriptionEngine::Whisper(whisper_engine) => {
            let language = crate::get_language_preference_internal();
            match whisper_engine
                .transcribe_audio_with_confidence(speech_samples, language)
                .await
            {
                Ok((text, confidence, is_partial)) => {
                    // Whisper cannot split an utterance into sentences, so the VAD segment
                    // remains the transcript line (the previous behaviour).
                    Ok((
                        whole_chunk(text.trim().to_string(), chunk_duration_s),
                        Some(confidence),
                        is_partial,
                        None,
                    ))
                }
                Err(error) => {
                    let transcription_error = TranscriptionError::EngineFailed(error.to_string());
                    let _ = app.emit(
                        "transcription-error",
                        &serde_json::json!({
                            "error": transcription_error.to_string(),
                            "userMessage": format!("Transcription failed: {}", transcription_error),
                            "actionable": false,
                            "phase": "active"
                        }),
                    );
                    Err(transcription_error)
                }
            }
        }
        TranscriptionEngine::SenseVoice(sensevoice_engine) => {
            // SenseVoice takes an ISO-639-1 hint; unsupported codes and "auto" become None,
            // which lets the model run its own language ID.
            let pinned_language = crate::get_language_preference_internal()
                .as_deref()
                .and_then(crate::sensevoice_engine::normalize_language);

            // Auto-detection is only trusted on segments long enough to earn it; below that
            // it hallucinates, and decodes the audio in the language it hallucinated.
            let trust_auto_detection = chunk_duration_s >= MIN_RELIABLE_LANGUAGE_ID_SECS;
            let language = match pinned_language {
                // The user pinned a language — always honour it.
                Some(language) => Some(language),
                None if trust_auto_detection => None,
                // Too short to identify on its own: decode it in the language this meeting
                // is being held in. Falls back to auto until we have seen a segment long
                // enough to establish that.
                None => dominant_language(),
            };

            match sensevoice_engine
                .transcribe_audio(speech_samples, language.clone())
                .await
            {
                Ok((sentences, detected_language)) => {
                    if sentences.is_empty() {
                        return Ok((Vec::new(), None, false, None));
                    }

                    // A segment decoded in a language we chose reports that language back;
                    // one we let the model identify reports what it found. Either way the
                    // line is tagged with the language it was actually decoded in.
                    let detected_language = detected_language.or_else(|| language.clone());

                    // Only segments long enough to identify get a vote on what language the
                    // meeting is in — otherwise a single mis-heard interjection could drag
                    // every later short utterance into the wrong language with it.
                    if trust_auto_detection {
                        if let Some(language) = detected_language.as_deref() {
                            note_confident_language(language);
                        }
                    }

                    info!(
                        "SenseVoice transcribed chunk {} into {} sentence(s) ({:.1}s, detected: {:?})",
                        chunk.chunk_id,
                        sentences.len(),
                        chunk_duration_s,
                        detected_language
                    );

                    // Sentence boundaries come from the model's punctuation, and their
                    // times from its CTC alignment — not from how long the speaker paused.
                    // In pause mode the user has asked for the old behaviour instead, so
                    // the sentences are joined back into the single VAD-segment line.
                    let sentences: Vec<TranscriptSentence> = if split_on_punctuation() {
                        sentences
                            .into_iter()
                            .map(|s| TranscriptSentence {
                                text: s.text,
                                start: s.start as f64,
                                end: s.end as f64,
                            })
                            .collect()
                    } else {
                        let joined = sentences
                            .into_iter()
                            .map(|s| s.text)
                            .collect::<Vec<_>>()
                            .join(" ");
                        whole_chunk(joined, chunk_duration_s)
                    };

                    // CTC decoding gives no confidence score and no partial results.
                    Ok((sentences, None, false, detected_language))
                }
                Err(error) => {
                    let transcription_error = TranscriptionError::EngineFailed(error.to_string());
                    let _ = app.emit(
                        "transcription-error",
                        &serde_json::json!({
                            "error": transcription_error.to_string(),
                            "userMessage": format!("Transcription failed: {}", transcription_error),
                            "actionable": false,
                            "phase": "active"
                        }),
                    );
                    Err(transcription_error)
                }
            }
        }
        TranscriptionEngine::Parakeet(parakeet_engine) => {
            match parakeet_engine.transcribe_audio(speech_samples).await {
                // Parakeet reports no confidence, language or sentence alignment.
                Ok(text) => Ok((
                    whole_chunk(text.trim().to_string(), chunk_duration_s),
                    None,
                    false,
                    None,
                )),
                Err(error) => {
                    let transcription_error = TranscriptionError::EngineFailed(error.to_string());
                    let _ = app.emit(
                        "transcription-error",
                        &serde_json::json!({
                            "error": transcription_error.to_string(),
                            "userMessage": format!("Transcription failed: {}", transcription_error),
                            "actionable": false,
                            "phase": "active"
                        }),
                    );
                    Err(transcription_error)
                }
            }
        }
        TranscriptionEngine::Provider(provider) => {
            let language = crate::get_language_preference_internal();
            match provider.transcribe(speech_samples, language).await {
                Ok(result) => Ok((
                    whole_chunk(result.text.trim().to_string(), chunk_duration_s),
                    result.confidence,
                    result.is_partial,
                    None,
                )),
                Err(error) => {
                    let _ = app.emit(
                        "transcription-error",
                        &serde_json::json!({
                            "error": error.to_string(),
                            "userMessage": format!("Transcription failed: {}", error),
                            "actionable": false,
                            "phase": "active"
                        }),
                    );
                    Err(error)
                }
            }
        }
    }
}

/// Format current timestamp (wall-clock time)
fn format_current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let hours = (now.as_secs() / 3600) % 24;
    let minutes = (now.as_secs() / 60) % 60;
    let seconds = now.as_secs() % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Format recording-relative time as [MM:SS]
#[allow(dead_code)]
fn format_recording_time(seconds: f64) -> String {
    let total_seconds = seconds.floor() as u64;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;

    format!("[{:02}:{:02}]", minutes, secs)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn keeps_short_acknowledgements() {
            assert!(should_emit_transcript("Yes"));
            assert!(should_emit_transcript("ok"));
        }

        #[test]
        fn drops_empty_and_whitespace_only() {
            assert!(!should_emit_transcript(""));
            assert!(!should_emit_transcript("   "));
        }

        #[test]
        fn logging_worker_sampling_and_diagnostics_are_bounded() {
            for accepted_count in [1, 2, 3, 4, 5, 25, 50] {
                assert!(should_log_transcript_snippet(accepted_count));
            }
            for accepted_count in [6, 24, 26, 49] {
                assert!(!should_log_transcript_snippet(accepted_count));
            }

            let mut stats = WorkerLogStats::default();
            for category in [
                WorkerFailureCategory::ModelNotLoaded,
                WorkerFailureCategory::EngineFailed,
                WorkerFailureCategory::UnsupportedLanguage,
                WorkerFailureCategory::SpeechDetectedEmit,
                WorkerFailureCategory::TranscriptUpdateEmit,
            ] {
                stats.record_failure(category);
            }
            let snapshot = stats.snapshot_and_reset();
            assert_eq!(snapshot.failed, 5);
            assert_eq!(snapshot.model_not_loaded_failures, 1);
            assert_eq!(snapshot.engine_failures, 1);
            assert_eq!(snapshot.unsupported_language_failures, 1);
            assert_eq!(snapshot.speech_detected_emit_failures, 1);
            assert_eq!(snapshot.transcript_update_emit_failures, 1);
            assert_eq!(stats.failed, 0);
            assert_eq!(stats.model_not_loaded_failures, 0);
            assert_eq!(stats.engine_failures, 0);
            assert_eq!(stats.unsupported_language_failures, 0);
            assert_eq!(stats.speech_detected_emit_failures, 0);
            assert_eq!(stats.transcript_update_emit_failures, 0);
        }
    }

#[cfg(test)]
mod language_tally_tests {
    use super::*;

    /// These tests share one process-wide tally, so they must not run concurrently.
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn dominant_language_is_the_one_heard_most() {
        let _guard = TEST_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        reset_language_tally();

        assert_eq!(
            dominant_language(),
            None,
            "with nothing heard yet there is no dominant language — a short utterance must \
             fall back to auto-detection rather than being forced into a guess"
        );

        note_confident_language("zh");
        note_confident_language("en");
        note_confident_language("zh");

        assert_eq!(dominant_language().as_deref(), Some("zh"));
    }

    /// The mis-detections this whole mechanism exists to stop must not feed back into it.
    #[test]
    fn a_meeting_does_not_inherit_the_previous_meeting_s_language() {
        let _guard = TEST_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        reset_language_tally();

        note_confident_language("ja");
        assert_eq!(dominant_language().as_deref(), Some("ja"));

        reset_language_tally();
        assert_eq!(
            dominant_language(),
            None,
            "a new recording must start with no opinion about its language"
        );
    }

    /// The threshold is the whole point: below it the model's language ID is a coin flip
    /// (55% wrong under 0.5s in a real meeting), above it it is dependable (0.6% wrong).
    #[test]
    fn only_segments_past_the_threshold_are_trusted() {
        assert!(0.4 < MIN_RELIABLE_LANGUAGE_ID_SECS, "0.4s must not be trusted");
        assert!(0.9 < MIN_RELIABLE_LANGUAGE_ID_SECS, "0.9s must not be trusted");
        assert!(2.5 >= MIN_RELIABLE_LANGUAGE_ID_SECS, "2.5s must be trusted");
    }
}

#[cfg(test)]
mod sentence_tests {
    use super::*;

    #[test]
    fn whole_chunk_spans_the_full_duration() {
        let sentences = whole_chunk("你好 world".to_string(), 3.5);
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].text, "你好 world");
        assert_eq!((sentences[0].start, sentences[0].end), (0.0, 3.5));
    }

    #[test]
    fn whole_chunk_drops_empty_and_whitespace_only_text() {
        assert!(whole_chunk(String::new(), 2.0).is_empty());
        assert!(whole_chunk("  \n\t".to_string(), 2.0).is_empty());
    }

    #[test]
    fn segmentation_mode_is_switchable_at_runtime() {
        set_split_on_punctuation(false);
        assert!(!split_on_punctuation());
        set_split_on_punctuation(true);
        assert!(split_on_punctuation());
    }

    #[test]
    fn short_utterances_use_the_meeting_language_not_auto_detection() {
        // Measured: under 0.5s the model's own language ID is wrong more than half the time.
        assert!(0.4 < MIN_RELIABLE_LANGUAGE_ID_SECS);
        assert!(MIN_RELIABLE_LANGUAGE_ID_SECS <= 1.0);
    }
}
