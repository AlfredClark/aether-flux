use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, ensure, Context, Result};
use ort::{
    ep::{self, ExecutionProvider},
    session::{
        builder::{GraphOptimizationLevel, SessionBuilder},
        Session,
    },
};
use tauri::{AppHandle, Manager};

/// ONNX Runtime 的执行设备选择策略。
///
/// `Auto` 会优先尝试 CUDA，失败后自动回退到 CPU。
///
/// `CpuOnly` 则完全跳过 GPU 探测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Auto,
    CpuOnly,
}

/// 各类 ASR 模型共享的 ONNX Runtime 初始化参数。
#[derive(Debug, Clone)]
pub struct OrtRuntimeConfig {
    pub intra_threads: usize,
    pub inter_threads: usize,
    pub optimization_level: GraphOptimizationLevel,
    pub execution_mode: ExecutionMode,
    pub gpu_device_id: i32,
}

impl Default for OrtRuntimeConfig {
    /// 返回统一的默认 ORT 配置，供各模型配置复用。
    fn default() -> Self {
        Self {
            intra_threads: 1,
            inter_threads: 1,
            optimization_level: GraphOptimizationLevel::Level3,
            execution_mode: ExecutionMode::Auto,
            gpu_device_id: 0,
        }
    }
}

/// 提供共享 ORT 运行时配置的抽象，便于不同模型复用同一套 session 构建逻辑。
pub trait OrtRuntimeConfigProvider {
    /// 暴露当前模型对应的共享 ORT 运行时配置。
    fn ort_runtime_config(&self) -> &OrtRuntimeConfig;
}

/// 统一抽象不同 ASR 模型的识别入口，调用方无需关心具体实现类型。
pub trait AsrLoader: Send {
    /// 识别指定 wav 文件并返回最终文本。
    fn recognize_wav_text(&mut self, wav_path: &Path) -> Result<String>;
}

pub type DynAsrLoader = Box<dyn AsrLoader>;

/// 按统一策略创建 ONNX Session，并在可用时自动启用 CUDA。
pub fn build_session(
    config: &impl OrtRuntimeConfigProvider,
    path: impl AsRef<Path>,
) -> Result<Session> {
    let path = path.as_ref();
    let runtime = config.ort_runtime_config();

    let builder = Session::builder().map_err(ort_error)?;
    let builder = builder
        .with_optimization_level(runtime.optimization_level)
        .map_err(ort_builder_error)?;
    let builder = builder
        .with_intra_threads(runtime.intra_threads)
        .map_err(ort_builder_error)?;
    let builder = builder
        .with_inter_threads(runtime.inter_threads)
        .map_err(ort_builder_error)?;
    let builder = builder
        .with_memory_pattern(false)
        .map_err(ort_builder_error)?;
    let mut builder = configure_execution_providers(builder, runtime)?;

    builder
        .commit_from_file(path)
        .map_err(ort_error)
        .with_context(|| format!("failed to load ONNX model from {}", path.display()))
}

/// 读取并反序列化 JSON 文件。
pub fn read_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    serde_json::from_reader(std::io::BufReader::new(file))
        .with_context(|| format!("failed to parse {}", path.display()))
}

/// 统一在多个候选目录中解析模型根目录。
///
/// 搜索顺序为：
/// 1. `app_local_data_dir/<relative>`
/// 2. `local_data_dir/<relative>`
/// 3. 开发环境 `resources/<relative>`
pub fn resolve_model_root<F>(
    app: &AppHandle,
    relative: &str,
    validate: F,
) -> Result<PathBuf, String>
where
    F: Fn(&Path) -> bool,
{
    let app_local_data = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|dir| dir.join(relative));
    let local_data = app
        .path()
        .local_data_dir()
        .ok()
        .map(|dir| dir.join(relative));
    let dev = Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(relative),
    );

    for candidate in [app_local_data, local_data, dev].into_iter().flatten() {
        if validate(&candidate) {
            return Ok(candidate);
        }
    }

    Err(format!(
        "no valid ASR model directory found for {relative}; checked app_local_data_dir, local_data_dir, and development resources"
    ))
}

/// 根据执行模式为 `SessionBuilder` 追加合适的执行提供者。
fn configure_execution_providers(
    builder: SessionBuilder,
    config: &OrtRuntimeConfig,
) -> Result<SessionBuilder> {
    match config.execution_mode {
        ExecutionMode::CpuOnly => Ok(builder),
        ExecutionMode::Auto => {
            let cuda = ep::CUDA::default().with_device_id(config.gpu_device_id);
            match cuda.is_available() {
                Ok(true) => builder
                    .with_execution_providers([cuda.build()])
                    .map_err(ort_builder_error),
                Ok(false) | Err(_) => Ok(builder),
            }
        }
    }
}

/// 将 ORT 通用错误包装为 `anyhow::Error`。
fn ort_error(error: ort::Error) -> anyhow::Error {
    anyhow!(error.to_string())
}

/// 将 ORT `SessionBuilder` 相关错误包装为 `anyhow::Error`。
fn ort_builder_error(error: ort::Error<SessionBuilder>) -> anyhow::Error {
    anyhow!(error.to_string())
}

/// 加载 wav 文件并在必要时混合为单声道采样。
pub(crate) fn load_wav_mono(path: &Path, expected_sample_rate: usize) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open wav file {}", path.display()))?;
    let spec = reader.spec();
    ensure!(
        spec.sample_rate as usize == expected_sample_rate,
        "expected {expected_sample_rate} Hz wav, got {} Hz for {}",
        spec.sample_rate,
        path.display()
    );
    ensure!(
        spec.channels >= 1,
        "wav file had zero channels: {}",
        path.display()
    );

    let channels = usize::from(spec.channels);
    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("failed to read f32 wav samples")?,
        (hound::SampleFormat::Int, bits_per_sample) if bits_per_sample <= 16 => {
            let scale = f32::from(i16::MAX);
            reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| f32::from(value) / scale))
                .collect::<Result<Vec<_>, _>>()
                .context("failed to read i16 wav samples")?
        }
        (hound::SampleFormat::Int, bits_per_sample) if bits_per_sample <= 32 => {
            let scale = ((1_i64 << (bits_per_sample - 1)) - 1) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .context("failed to read i32 wav samples")?
        }
        _ => bail!(
            "unsupported wav format for {}: {:?} {}-bit",
            path.display(),
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    if channels == 1 {
        return Ok(samples);
    }

    let mut mono = Vec::with_capacity(samples.len() / channels);
    for frame in samples.chunks_exact(channels) {
        mono.push(frame.iter().sum::<f32>() / channels as f32);
    }
    Ok(mono)
}

/// 在贪心解码时返回最大值对应的下标。
pub(crate) fn argmax(values: ndarray::ArrayView1<'_, f32>) -> usize {
    let mut best_idx = 0usize;
    let mut best_value = f32::NEG_INFINITY;
    for (idx, value) in values.iter().copied().enumerate() {
        if value > best_value {
            best_idx = idx;
            best_value = value;
        }
    }
    best_idx
}
