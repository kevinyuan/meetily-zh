use std::path::PathBuf;

/// Errors produced by transcribe-rs engines.
#[derive(Debug, thiserror::Error)]
pub enum TranscribeError {
    #[error("model not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("inference error: {0}")]
    Inference(String),

    #[error("audio error: {0}")]
    Audio(String),

    #[error("config error: {0}")]
    Config(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

// ---- From impls for common error types ----
// Upstream's `From<hound::Error>` is dropped along with the WAV-reading module.

impl From<ort::Error> for TranscribeError {
    fn from(e: ort::Error) -> Self {
        TranscribeError::Inference(e.to_string())
    }
}

impl From<ndarray::ShapeError> for TranscribeError {
    fn from(e: ndarray::ShapeError) -> Self {
        TranscribeError::Inference(e.to_string())
    }
}
