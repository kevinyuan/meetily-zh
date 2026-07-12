// PORT NOTE (ort rc.12 -> rc.10):
//   rc.12: `use ort::ep::CPU;`  and  `ort::ep::ExecutionProviderDispatch`
//   rc.10: the `ort::ep` module does not exist; EPs live in
//          `ort::execution_providers` and are named `<Name>ExecutionProvider`.
use ort::execution_providers::{CPUExecutionProvider, ExecutionProviderDispatch};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use std::path::Path;

/// Build the execution provider list.
///
/// TRIMMED (not an rc.10 issue): upstream consults `crate::accel::get_ort_accelerator()`
/// and pushes CUDA/TensorRT/DirectML/ROCm/CoreML/WebGPU/XNNPACK dispatches behind cargo
/// features. This proof crate is CPU-only, so only the CPU fallback remains.
fn execution_providers() -> Vec<ExecutionProviderDispatch> {
    // CPU is always the final fallback
    vec![CPUExecutionProvider::default().build()]
}

/// Internal session builder with full control over threading and EP selection.
fn build_session(
    path: &Path,
    intra_threads: Option<usize>,
    parallel_execution: bool,
) -> Result<Session, ort::Error> {
    let mut builder =
        Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?;

    if let Some(n) = intra_threads {
        if n > 0 {
            builder = builder.with_intra_threads(n)?;
        }
    }

    builder = builder.with_parallel_execution(parallel_execution)?;

    let session = builder
        .with_execution_providers(execution_providers())?
        .commit_from_file(path)?;

    // PORT NOTE (ort rc.12 -> rc.10):
    //   rc.12: `session.inputs()` / `session.outputs()` are methods returning slices of
    //          `Input`/`Output`, whose name & type are read via `.name()` / `.dtype()`.
    //   rc.10: `Session::inputs` / `Session::outputs` are public *fields* (`Vec<Input>` /
    //          `Vec<Output>`), and `Input`/`Output` expose public fields `name: String`
    //          and `input_type` / `output_type: ValueType`.
    for input in &session.inputs {
        log::info!(
            "Model input: name={}, type={:?}",
            input.name,
            input.input_type
        );
    }
    for output in &session.outputs {
        log::info!(
            "Model output: name={}, type={:?}",
            output.name,
            output.output_type
        );
    }

    Ok(session)
}

/// Create an ONNX session with standard settings.
pub fn create_session(path: &Path) -> Result<Session, ort::Error> {
    build_session(path, None, true)
}

/// Create an ONNX session with configurable thread count.
#[allow(dead_code)]
pub fn create_session_with_threads(path: &Path, num_threads: usize) -> Result<Session, ort::Error> {
    build_session(path, Some(num_threads), true)
}

/// Resolve a model file path for the requested quantization level.
///
/// Looks for `{name}.{suffix}.onnx` based on the quantization variant,
/// falling back to `{name}.onnx` (FP32) if the requested file doesn't exist.
pub fn resolve_model_path(
    dir: &Path,
    name: &str,
    quantization: &super::Quantization,
) -> std::path::PathBuf {
    let suffix = match quantization {
        super::Quantization::FP32 => None,
        super::Quantization::FP16 => Some("fp16"),
        super::Quantization::Int8 => Some("int8"),
        super::Quantization::Int4 => Some("int4"),
    };

    if let Some(suffix) = suffix {
        let path = dir.join(format!("{}.{}.onnx", name, suffix));
        if path.exists() {
            log::info!("Loading {} model: {}", suffix, path.display());
            return path;
        }
        log::warn!(
            "{} model not found at {}, falling back to {}.onnx",
            suffix,
            path.display(),
            name
        );
    }

    dir.join(format!("{}.onnx", name))
}

/// Read a custom metadata string from an ONNX session.
///
/// PORT NOTE (ort rc.12 -> rc.10):
///   rc.12: `ModelMetadata::custom(&self, key) -> Option<String>`  (infallible)
///          upstream body: `Ok(meta.custom(key).filter(|s| !s.is_empty()))`
///   rc.10: `ModelMetadata::custom(&self, key) -> ort::Result<Option<String>>`
///          so a `?` must be added before `.filter(..)`.
pub fn read_metadata_str(session: &Session, key: &str) -> Result<Option<String>, ort::Error> {
    let meta = session.metadata()?;
    Ok(meta.custom(key)?.filter(|s| !s.is_empty()))
}

/// Read a custom metadata i32 value, with optional default.
pub fn read_metadata_i32(
    session: &Session,
    key: &str,
    default: Option<i32>,
) -> Result<Option<i32>, crate::sensevoice_engine::vendor::TranscribeError> {
    let str_val = read_metadata_str(session, key).map_err(|e| {
        crate::sensevoice_engine::vendor::TranscribeError::Config(format!("failed to read metadata '{}': {}", key, e))
    })?;
    match str_val {
        Some(v) => Ok(Some(v.parse::<i32>().map_err(|e| {
            crate::sensevoice_engine::vendor::TranscribeError::Config(format!("failed to parse '{}': {}", key, e))
        })?)),
        None => Ok(default),
    }
}

/// Read a comma-separated float vector from metadata.
pub fn read_metadata_float_vec(
    session: &Session,
    key: &str,
) -> Result<Option<Vec<f32>>, crate::sensevoice_engine::vendor::TranscribeError> {
    let str_val = read_metadata_str(session, key).map_err(|e| {
        crate::sensevoice_engine::vendor::TranscribeError::Config(format!("failed to read metadata '{}': {}", key, e))
    })?;
    match str_val {
        Some(v) => {
            let floats: Result<Vec<f32>, _> =
                v.split(',').map(|s| s.trim().parse::<f32>()).collect();
            Ok(Some(floats.map_err(|e| {
                crate::sensevoice_engine::vendor::TranscribeError::Config(format!(
                    "failed to parse floats in '{}': {}",
                    key, e
                ))
            })?))
        }
        None => Ok(None),
    }
}
