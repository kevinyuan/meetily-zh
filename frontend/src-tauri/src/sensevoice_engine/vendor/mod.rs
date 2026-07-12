//! Minimal vendored subset of transcribe-rs 0.3.11 (MIT, (c) 2025 Ilya Stupakov)
//! — SenseVoice only. See vendor/LICENSE.
//!
//! Upstream targets ort 2.0.0-rc.12 + ndarray 0.17. We cannot: silero_rs pins
//! `ort = "=2.0.0-rc.10"` exactly, and rc.12 makes `ort::Error` !Send + !Sync,
//! which breaks silero's and Parakeet's `#[from] ort::Error` enums. So this is
//! ported back to rc.10 + ndarray 0.16. The port is 4 changes across 2 files
//! (session.rs, sense_voice/mod.rs); everything else is byte-identical upstream.
//!
//! Upstream's `audio` module (WAV reading, via hound) is intentionally omitted —
//! meetily feeds f32 samples straight from its own audio pipeline.

pub mod error;
pub use error::TranscribeError;

pub mod decode;
pub mod features;
pub mod onnx;

/// 16 kHz mono — the sample rate every engine here expects.
const SAMPLES_PER_MS: usize = 16;

/// Describes the capabilities of a speech model.
#[derive(Debug, Clone)]
pub struct ModelCapabilities {
    /// Human-readable model name.
    pub name: &'static str,
    /// Machine-friendly engine identifier (e.g. "sense_voice", "whisper_cpp").
    pub engine_id: &'static str,
    /// Expected input sample rate in Hz (e.g. 16000).
    pub sample_rate: u32,
    /// Languages supported (BCP-47 codes, e.g. "en", "zh"). Empty = any/unknown.
    pub languages: &'static [&'static str],
    /// Whether the model can produce word/segment timestamps.
    pub supports_timestamps: bool,
    /// Whether the model can translate to English.
    pub supports_translation: bool,
    /// Whether the model supports streaming inference.
    pub supports_streaming: bool,
}

/// Options for transcription.
#[derive(Debug, Clone, Default)]
pub struct TranscribeOptions {
    /// Language hint (BCP-47 code, e.g. "en", "zh").
    pub language: Option<String>,
    /// Whether to translate the output to English (only supported by some engines).
    pub translate: bool,
    /// Leading silence padding in milliseconds prepended before audio.
    pub leading_silence_ms: Option<u32>,
    /// Trailing silence padding in milliseconds appended after audio.
    pub trailing_silence_ms: Option<u32>,
}

/// Unified interface for speech-to-text models.
pub trait SpeechModel: Send {
    /// Report this model's capabilities.
    fn capabilities(&self) -> ModelCapabilities;

    /// Default leading silence in milliseconds for this engine.
    fn default_leading_silence_ms(&self) -> u32 {
        0
    }

    /// Default trailing silence in milliseconds for this engine.
    fn default_trailing_silence_ms(&self) -> u32 {
        0
    }

    /// Raw transcription — engines implement this with their inference logic.
    fn transcribe_raw(
        &mut self,
        samples: &[f32],
        options: &TranscribeOptions,
    ) -> Result<TranscriptionResult, TranscribeError>;

    /// Transcribe audio samples (16 kHz, mono, f32 in [-1, 1]).
    fn transcribe(
        &mut self,
        samples: &[f32],
        options: &TranscribeOptions,
    ) -> Result<TranscriptionResult, TranscribeError> {
        let lead_ms = options
            .leading_silence_ms
            .unwrap_or_else(|| self.default_leading_silence_ms());
        let trail_ms = options
            .trailing_silence_ms
            .unwrap_or_else(|| self.default_trailing_silence_ms());

        // Fast path: no padding needed.
        if lead_ms == 0 && trail_ms == 0 {
            return self.transcribe_raw(samples, options);
        }

        let mut buf = if lead_ms > 0 {
            let lead_len = lead_ms as usize * SAMPLES_PER_MS;
            let mut padded = vec![0.0; lead_len];
            padded.extend_from_slice(samples);
            padded
        } else {
            samples.to_vec()
        };
        if trail_ms > 0 {
            let trail_len = trail_ms as usize * SAMPLES_PER_MS;
            buf.resize(buf.len() + trail_len, 0.0);
        }

        let mut result = self.transcribe_raw(&buf, options)?;

        if lead_ms > 0 {
            result.offset_timestamps(-(lead_ms as f32 / 1000.0));
        }

        Ok(result)
    }
}

/// The result of a transcription operation.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// The complete transcribed text from the audio
    pub text: String,
    /// Individual segments with timing information
    pub segments: Option<Vec<TranscriptionSegment>>,
}

impl TranscriptionResult {
    /// Shift all segment timestamps by `offset_secs`, clamping to zero.
    pub fn offset_timestamps(&mut self, offset_secs: f32) {
        if let Some(segs) = &mut self.segments {
            for seg in segs {
                seg.start = (seg.start + offset_secs).max(0.0);
                seg.end = (seg.end + offset_secs).max(0.0);
            }
        }
    }
}

/// A single transcribed segment with timing information.
#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    /// Start time of the segment in seconds
    pub start: f32,
    /// End time of the segment in seconds
    pub end: f32,
    /// The transcribed text for this segment
    pub text: String,
}
