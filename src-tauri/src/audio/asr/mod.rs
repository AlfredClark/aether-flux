mod qwen3_asr;
mod sensevoice_small;
mod utils;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::{async_runtime, AppHandle, Manager, State};

use self::{
    qwen3_asr::{
        ExecutionMode as Qwen3ExecutionMode, Qwen3AsrLoader, Qwen3AsrLoaderConfig,
    },
    sensevoice_small::{
        ExecutionMode as SenseVoiceExecutionMode, SenseVoiceSmallLoader, SenseVoiceSmallLoaderConfig,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrModelKind {
    Qwen3Asr,
    #[serde(alias = "sensevoice_small")]
    SenseVoiceSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrExecutionMode {
    Auto,
    OnlyCpu,
}

#[derive(Default)]
pub struct AsrState {
    /// 当前已加载的 ASR 运行时，放在 Mutex 中以便命令串行访问和安全替换。
    pub(crate) inner: Arc<Mutex<AsrStateInner>>,
}

#[derive(Default)]
pub(crate) struct AsrStateInner {
    pub(crate) current_model: Option<AsrModelKind>,
    pub(crate) current_mode: Option<AsrExecutionMode>,
    pub(crate) runtime: Option<AsrRuntime>,
}

pub(crate) enum AsrRuntime {
    Qwen3(Qwen3AsrLoader),
    SenseVoiceSmall(SenseVoiceSmallLoader),
}

impl AsrRuntime {
    /// 统一封装不同模型加载器的识别入口，前端不需要感知具体实现差异。
    fn recognize_wav_text(&mut self, wav_path: &Path) -> Result<String, String> {
        match self {
            Self::Qwen3(loader) => loader
                .recognize_wav_text(wav_path)
                .map_err(|err| format!("Qwen3-ASR 识别失败: {err:#}")),
            Self::SenseVoiceSmall(loader) => loader
                .recognize_wav_text(wav_path)
                .map_err(|err| format!("SenseVoiceSmall 识别失败: {err:#}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStatus {
    pub is_loaded: bool,
    pub current_model: Option<AsrModelKind>,
    pub current_mode: Option<AsrExecutionMode>,
}

impl AsrStatus {
    fn from_inner(inner: &AsrStateInner) -> Self {
        Self {
            is_loaded: inner.runtime.is_some(),
            current_model: inner.current_model,
            current_mode: inner.current_mode,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrRecognitionResult {
    pub text: String,
    pub model: AsrModelKind,
    pub audio_path: String,
}

#[tauri::command]
pub fn get_asr_status(asr_state: State<'_, AsrState>) -> Result<AsrStatus, String> {
    let guard = asr_state
        .inner
        .lock()
        .map_err(|_| "Failed to lock ASR state".to_string())?;
    Ok(AsrStatus::from_inner(&guard))
}

/// 页面加载或用户切换模型时调用，旧模型会在替换时自动释放。
#[tauri::command]
pub async fn load_asr_model(
    model: AsrModelKind,
    mode: AsrExecutionMode,
    app: AppHandle,
    asr_state: State<'_, AsrState>,
) -> Result<AsrStatus, String> {
    let state = Arc::clone(&asr_state.inner);
    let runtime = async_runtime::spawn_blocking(move || build_runtime(&app, model, mode))
        .await
        .map_err(|err| format!("Failed to join ASR loader task: {err}"))??;

    let mut guard = state
        .lock()
        .map_err(|_| "Failed to lock ASR state".to_string())?;
    guard.current_model = Some(model);
    guard.current_mode = Some(mode);
    guard.runtime = Some(runtime);

    Ok(AsrStatus::from_inner(&guard))
}

/// 页面退出时主动销毁模型，避免 ONNX Session 长时间占用内存和显存。
#[tauri::command]
pub fn destroy_asr_model(asr_state: State<'_, AsrState>) -> Result<(), String> {
    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| "Failed to lock ASR state".to_string())?;
    guard.current_model = None;
    guard.current_mode = None;
    guard.runtime = None;
    Ok(())
}

/// 停止录音后立即调用该命令执行离线识别，并把文本结果回传给前端页面。
#[tauri::command]
pub async fn recognize_audio(
    wav_path: String,
    asr_state: State<'_, AsrState>,
) -> Result<AsrRecognitionResult, String> {
    let state = Arc::clone(&asr_state.inner);
    async_runtime::spawn_blocking(move || {
        let wav_path = PathBuf::from(wav_path);
        let mut guard = state
            .lock()
            .map_err(|_| "Failed to lock ASR state".to_string())?;
        let model = guard
            .current_model
            .ok_or_else(|| "ASR model is not initialized".to_string())?;
        let runtime = guard
            .runtime
            .as_mut()
            .ok_or_else(|| "ASR model is not loaded".to_string())?;
        let text = runtime.recognize_wav_text(&wav_path)?;

        Ok(AsrRecognitionResult {
            text,
            model,
            audio_path: wav_path.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|err| format!("Failed to join ASR recognition task: {err}"))?
}

fn build_runtime(app: &AppHandle, model: AsrModelKind, mode: AsrExecutionMode) -> Result<AsrRuntime, String> {
    match model {
        AsrModelKind::Qwen3Asr => {
            let root = resolve_model_root(app, "Qwen3-ASR-onnx")?;
            let mut config = Qwen3AsrLoaderConfig::from_root(root, "model_0.6B");
            config.execution_mode = match mode {
                AsrExecutionMode::Auto => Qwen3ExecutionMode::Auto,
                AsrExecutionMode::OnlyCpu => Qwen3ExecutionMode::CpuOnly,
            };
            let loader = Qwen3AsrLoader::new(config)
                .map_err(|err| format!("Qwen3-ASR 加载失败: {err:#}"))?;
            Ok(AsrRuntime::Qwen3(loader))
        }
        AsrModelKind::SenseVoiceSmall => {
            let root = resolve_model_root(app, "SenseVoiceSmall-onnx")?;
            let mut config = SenseVoiceSmallLoaderConfig::from_root(root);
            config.execution_mode = match mode {
                AsrExecutionMode::Auto => SenseVoiceExecutionMode::Auto,
                AsrExecutionMode::OnlyCpu => SenseVoiceExecutionMode::CpuOnly,
            };
            let loader = SenseVoiceSmallLoader::new(config)
                .map_err(|err| format!("SenseVoiceSmall 加载失败: {err:#}"))?;
            Ok(AsrRuntime::SenseVoiceSmall(loader))
        }
    }
}

fn resolve_model_root(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    // 生产环境优先从本地数据目录读取模型。
    // 这里同时兼容两种常见放置方式：
    // 1. Tauri 应用专属目录: $APPLOCALDATA/<bundle-id>/...
    // 2. 用户直接放在系统本地数据目录: $APPLOCALDATA/...
    let app_local_data = app.path().app_local_data_dir().ok().map(|dir| dir.join(relative));
    let local_data = app.path().local_data_dir().ok().map(|dir| dir.join(relative));
    let dev = Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(relative),
    );

    for candidate in [app_local_data, local_data, dev].into_iter().flatten() {
        if model_root_is_valid(relative, &candidate) {
            return Ok(candidate);
        }
    }

    Err(format!(
        "未找到可用的 ASR 模型资源目录: {relative}，已检查 app_local_data_dir、local_data_dir 和开发环境 resources 目录"
    ))
}

fn model_root_is_valid(relative: &str, root: &Path) -> bool {
    match relative {
        "Qwen3-ASR-onnx" => root.join("model_0.6B").is_dir() && root.join("tokenizer").is_dir(),
        "SenseVoiceSmall-onnx" => {
            root.join("model_quant.onnx").is_file()
                && root.join("tokens.json").is_file()
                && root.join("am.mvn").is_file()
        }
        _ => root.exists(),
    }
}
