/// Application configuration constants
///
/// Centralized definitions for default models and settings.
/// Used across database initialization, import, and retranscription.

/// Default Whisper model for transcription when no preference is configured.
/// This is the recommended balance of accuracy and speed.
pub const DEFAULT_WHISPER_MODEL: &str = "large-v3-turbo";

/// Default Parakeet model for transcription when no preference is configured.
/// This is the quantized version optimized for speed.
pub const DEFAULT_PARAKEET_MODEL: &str = "parakeet-tdt-0.6b-v3-int8";

/// Default SenseVoice model — the Chinese-optimised path.
pub const DEFAULT_SENSEVOICE_MODEL: &str = "sense-voice-small-int8";

/// Languages advertised for the model language filter.
///
/// Whisper is genuinely multilingual (99 languages), but the filter only offers
/// the languages the app itself ships a UI for, so tagging those is sufficient and
/// keeps the catalog honest about what the filter can act on.
pub const WHISPER_LANGUAGES: &[&str] = &["en", "zh"];

/// Parakeet v2/v3 are English-only for our purposes. v3 adds European languages,
/// but *not* Chinese — so it must never appear under the Chinese filter.
pub const PARAKEET_LANGUAGES: &[&str] = &["en"];

/// A downloadable Whisper model.
pub struct WhisperModelDef {
    pub name: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub accuracy: &'static str,
    pub speed: &'static str,
    pub description: &'static str,
    pub languages: &'static [&'static str],
}

/// Whisper model catalog — the single source of truth.
///
/// The download URL lives here rather than in a second `match` block inside
/// whisper_engine.rs, and the frontend reads this via `whisper_get_available_models`
/// rather than keeping its own copy of the metadata. Adding a model is a one-line
/// change here.
pub const WHISPER_MODEL_CATALOG: &[WhisperModelDef] = &[
    // Standard f16 models (full precision)
    WhisperModelDef {
        name: "tiny",
        filename: "ggml-tiny.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        size_mb: 74,
        accuracy: "Decent",
        speed: "Very Fast",
        description: "Fastest processing, good for real-time use",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "base",
        filename: "ggml-base.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        size_mb: 142,
        accuracy: "Good",
        speed: "Fast",
        description: "Good balance of speed and accuracy",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "small",
        filename: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        size_mb: 466,
        accuracy: "Good",
        speed: "Medium",
        description: "Better accuracy, moderate speed",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "medium",
        filename: "ggml-medium.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        size_mb: 1463,
        accuracy: "High",
        speed: "Slow",
        description: "High accuracy for professional use",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "large-v3-turbo",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        size_mb: 1549,
        accuracy: "High",
        speed: "Medium",
        description: "Best accuracy with improved speed",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "large-v3",
        filename: "ggml-large-v3.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        size_mb: 2951,
        accuracy: "High",
        speed: "Slow",
        description: "Most Accurate, latest large model",
        languages: WHISPER_LANGUAGES,
    },
    // Q5_1 quantized models (balanced speed/accuracy, slightly better quality than Q5_0)
    WhisperModelDef {
        name: "tiny-q5_1",
        filename: "ggml-tiny-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny-q5_1.bin",
        size_mb: 31,
        accuracy: "Decent",
        speed: "Very Fast",
        description: "Quantized tiny model, ~50% faster processing",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "base-q5_1",
        filename: "ggml-base-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin",
        size_mb: 57,
        accuracy: "Good",
        speed: "Fast",
        description: "Quantized base model, good speed/accuracy balance",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "small-q5_1",
        filename: "ggml-small-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        size_mb: 181,
        accuracy: "Good",
        speed: "Fast",
        description: "Quantized small model, faster than f16 version",
        languages: WHISPER_LANGUAGES,
    },
    // Q5_0 quantized models (balanced speed/accuracy)
    WhisperModelDef {
        name: "medium-q5_0",
        filename: "ggml-medium-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
        size_mb: 514,
        accuracy: "High",
        speed: "Medium",
        description: "Quantized medium model, professional quality",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "large-v3-turbo-q5_0",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        size_mb: 547,
        accuracy: "High",
        speed: "Medium",
        description: "Quantized large model, best balance",
        languages: WHISPER_LANGUAGES,
    },
    WhisperModelDef {
        name: "large-v3-q5_0",
        filename: "ggml-large-v3-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-q5_0.bin",
        size_mb: 1031,
        accuracy: "High",
        speed: "Slow",
        description: "Quantized large model, high accuracy",
        languages: WHISPER_LANGUAGES,
    },
];

/// Look up a Whisper model by name.
pub fn whisper_model(name: &str) -> Option<&'static WhisperModelDef> {
    WHISPER_MODEL_CATALOG.iter().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_names_and_filenames_are_unique() {
        let names: HashSet<_> = WHISPER_MODEL_CATALOG.iter().map(|m| m.name).collect();
        let files: HashSet<_> = WHISPER_MODEL_CATALOG.iter().map(|m| m.filename).collect();
        assert_eq!(names.len(), WHISPER_MODEL_CATALOG.len());
        assert_eq!(files.len(), WHISPER_MODEL_CATALOG.len());
    }

    #[test]
    fn every_download_url_is_https_and_ends_with_its_filename() {
        for model in WHISPER_MODEL_CATALOG {
            assert!(model.url.starts_with("https://"), "{} is not https", model.name);
            assert!(
                model.url.ends_with(model.filename),
                "{}: url {} does not end with {}",
                model.name, model.url, model.filename
            );
        }
    }

    #[test]
    fn every_model_has_a_plausible_size_and_metadata() {
        for model in WHISPER_MODEL_CATALOG {
            assert!(model.size_mb > 0, "{} has no size", model.name);
            assert!(!model.description.is_empty(), "{} has no description", model.name);
            assert!(!model.accuracy.is_empty() && !model.speed.is_empty());
        }
    }

    #[test]
    fn lookup_finds_known_models_and_rejects_unknown_ones() {
        assert_eq!(whisper_model("large-v3-turbo").map(|m| m.filename), Some("ggml-large-v3-turbo.bin"));
        assert!(whisper_model("does-not-exist").is_none());
        assert!(whisper_model("").is_none());
    }

    #[test]
    fn the_default_whisper_model_exists_in_the_catalog() {
        assert!(whisper_model(DEFAULT_WHISPER_MODEL).is_some());
    }

    /// Parakeet cannot transcribe Chinese. If this ever lists "zh", the Chinese
    /// language filter will offer a model that returns garbage.
    #[test]
    fn parakeet_is_never_offered_for_chinese() {
        assert!(!PARAKEET_LANGUAGES.contains(&"zh"));
        assert!(PARAKEET_LANGUAGES.contains(&"en"));
    }

    #[test]
    fn every_whisper_model_is_offered_for_both_chinese_and_english() {
        for model in WHISPER_MODEL_CATALOG {
            assert!(model.languages.contains(&"zh"), "{} hidden under the Chinese filter", model.name);
            assert!(model.languages.contains(&"en"), "{} hidden under the English filter", model.name);
        }
    }
}
