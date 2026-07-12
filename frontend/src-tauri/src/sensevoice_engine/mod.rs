//! SenseVoice speech recognition engine.
//!
//! The Chinese-optimised transcription path, alongside Whisper (multilingual) and
//! Parakeet (English-only). SenseVoice handles Chinese, English, Japanese, Korean
//! and Cantonese, decodes via CTC in a single pass (~60x realtime on CPU), and
//! runs on the same ONNX Runtime as Parakeet.
//!
//! # Module structure
//!
//! - `sensevoice_engine`: engine — model discovery, download, load, transcribe
//! - `commands`: Tauri command interface
//! - `vendor`: vendored SenseVoice inference code from transcribe-rs (MIT)

pub mod commands;
pub mod sensevoice_engine;
pub mod vendor;

pub use commands::*;
pub use sensevoice_engine::{
    normalize_language, DownloadProgress, ModelInfo, ModelStatus, SenseVoiceEngine,
    SENSEVOICE_LANGUAGES,
};
