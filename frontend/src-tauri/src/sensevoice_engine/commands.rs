//! Tauri command surface for the SenseVoice engine.
//!
//! Command names and event payloads deliberately mirror `parakeet_engine::commands`
//! (`sensevoice-model-download-{progress,complete,error}` with the same JSON shape),
//! so the shared download-toast components pick these up without special-casing.

use crate::sensevoice_engine::{DownloadProgress, ModelInfo, SenseVoiceEngine};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, Manager, Runtime};

pub static SENSEVOICE_ENGINE: Mutex<Option<Arc<SenseVoiceEngine>>> = Mutex::new(None);

static MODELS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Point the engine at `<app_data>/models`. Called once during app setup, before
/// `sensevoice_init`.
pub fn set_models_directory<R: Runtime>(app: &AppHandle<R>) {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        log::error!("SenseVoice: failed to resolve app data dir");
        return;
    };

    let models_dir = app_data_dir.join("models");

    if !models_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&models_dir) {
            log::error!("Failed to create models directory: {}", e);
            return;
        }
    }

    log::info!(
        "SenseVoice models directory set to: {}",
        models_dir.display()
    );
    *MODELS_DIR.lock().unwrap() = Some(models_dir);
}

fn get_models_directory() -> Option<PathBuf> {
    MODELS_DIR.lock().unwrap().clone()
}

fn engine() -> Option<Arc<SenseVoiceEngine>> {
    SENSEVOICE_ENGINE.lock().unwrap().as_ref().cloned()
}

fn require_engine() -> Result<Arc<SenseVoiceEngine>, String> {
    engine().ok_or_else(|| "SenseVoice engine not initialized".to_string())
}

#[command]
pub async fn sensevoice_init() -> Result<(), String> {
    let mut guard = SENSEVOICE_ENGINE.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }

    let engine = SenseVoiceEngine::new_with_models_dir(get_models_directory())
        .map_err(|e| format!("Failed to initialize SenseVoice engine: {}", e))?;
    *guard = Some(Arc::new(engine));
    Ok(())
}

#[command]
pub async fn sensevoice_get_available_models() -> Result<Vec<ModelInfo>, String> {
    require_engine()?
        .discover_models()
        .await
        .map_err(|e| format!("Failed to discover SenseVoice models: {}", e))
}

#[command]
pub async fn sensevoice_load_model<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    let engine = require_engine()?;

    let _ = app_handle.emit(
        "model-loading-started",
        serde_json::json!({ "modelName": model_name, "provider": "senseVoice" }),
    );

    match engine.load_model(&model_name).await {
        Ok(()) => {
            let _ = app_handle.emit(
                "model-loading-completed",
                serde_json::json!({ "modelName": model_name, "provider": "senseVoice" }),
            );
            Ok(())
        }
        Err(e) => {
            let error = format!("Failed to load SenseVoice model: {}", e);
            let _ = app_handle.emit(
                "model-loading-failed",
                serde_json::json!({ "modelName": model_name, "error": error }),
            );
            Err(error)
        }
    }
}

#[command]
pub async fn sensevoice_get_current_model() -> Result<Option<String>, String> {
    Ok(require_engine()?.get_current_model().await)
}

#[command]
pub async fn sensevoice_is_model_loaded() -> Result<bool, String> {
    Ok(require_engine()?.is_model_loaded().await)
}

#[command]
pub async fn sensevoice_has_available_models() -> Result<bool, String> {
    let models = require_engine()?
        .discover_models()
        .await
        .map_err(|e| format!("Failed to discover SenseVoice models: {}", e))?;

    Ok(models.iter().any(|m| {
        matches!(
            m.status,
            crate::sensevoice_engine::ModelStatus::Available
        )
    }))
}

/// Returns the name of an installed model, or an error explaining what to download.
#[command]
pub async fn sensevoice_validate_model_ready() -> Result<String, String> {
    let engine = require_engine()?;
    let models = engine
        .discover_models()
        .await
        .map_err(|e| format!("Failed to discover SenseVoice models: {}", e))?;

    models
        .iter()
        .find(|m| matches!(m.status, crate::sensevoice_engine::ModelStatus::Available))
        .map(|m| m.name.clone())
        .ok_or_else(|| {
            "No SenseVoice model installed. Download one in Settings > Transcription Models."
                .to_string()
        })
}

/// Validate *and load* the configured model, the way the recording pipeline needs it.
///
/// Mirrors `parakeet_validate_model_ready_with_config`: if a model is already
/// loaded, use it; otherwise load the one named in the transcript config, falling
/// back to any installed model.
pub async fn sensevoice_validate_model_ready_with_config<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<String, String> {
    let engine = require_engine()?;

    if engine.is_model_loaded().await {
        if let Some(current) = engine.get_current_model().await {
            log::info!("SenseVoice model already loaded: {}", current);
            return Ok(current);
        }
    }

    let configured = match crate::api::api::api_get_transcript_config(
        app.clone(),
        app.state(),
        None,
    )
    .await
    {
        Ok(Some(config)) if config.provider == "senseVoice" => Some(config.model),
        _ => None,
    };

    // Fall back to whatever is installed, so a user who downloaded a model but
    // never explicitly selected it can still record.
    let model_name = match configured {
        Some(name) => name,
        None => sensevoice_validate_model_ready().await?,
    };

    engine
        .load_model(&model_name)
        .await
        .map_err(|e| format!("Failed to load SenseVoice model '{}': {}", model_name, e))?;

    Ok(model_name)
}

#[command]
pub async fn sensevoice_transcribe_audio(
    audio_data: Vec<f32>,
    language: Option<String>,
) -> Result<String, String> {
    require_engine()?
        .transcribe_audio(audio_data, language)
        .await
        .map_err(|e| format!("SenseVoice transcription failed: {}", e))
}

#[command]
pub async fn sensevoice_get_models_directory() -> Result<String, String> {
    Ok(require_engine()?
        .get_models_directory()
        .await
        .to_string_lossy()
        .to_string())
}

#[command]
pub async fn sensevoice_download_model<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    let engine = require_engine()?;

    let progress_app = app_handle.clone();
    let progress_model = model_name.clone();

    let on_progress = move |progress: DownloadProgress| {
        let _ = progress_app.emit(
            "sensevoice-model-download-progress",
            serde_json::json!({
                "modelName": progress_model,
                "progress": progress.percent,
                "downloaded_bytes": progress.downloaded_bytes,
                "total_bytes": progress.total_bytes,
                "downloaded_mb": progress.downloaded_mb,
                "total_mb": progress.total_mb,
                "speed_mbps": progress.speed_mbps,
            }),
        );
    };

    match engine.download_model(&model_name, on_progress).await {
        Ok(()) => {
            let _ = app_handle.emit(
                "sensevoice-model-download-complete",
                serde_json::json!({ "modelName": model_name }),
            );
            Ok(())
        }
        Err(e) => {
            let error = e.to_string();
            let _ = app_handle.emit(
                "sensevoice-model-download-error",
                serde_json::json!({ "modelName": model_name, "error": error }),
            );
            Err(format!("Failed to download SenseVoice model: {}", error))
        }
    }
}

#[command]
pub async fn sensevoice_cancel_download(model_name: String) -> Result<(), String> {
    require_engine()?
        .cancel_download(&model_name)
        .await
        .map_err(|e| format!("Failed to cancel download: {}", e))
}

#[command]
pub async fn sensevoice_delete_model(model_name: String) -> Result<String, String> {
    require_engine()?
        .delete_model(&model_name)
        .await
        .map_err(|e| format!("Failed to delete SenseVoice model: {}", e))
}

#[command]
pub async fn open_sensevoice_models_folder() -> Result<(), String> {
    let dir = require_engine()?.get_models_directory().await;

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";

    std::process::Command::new(cmd)
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}
