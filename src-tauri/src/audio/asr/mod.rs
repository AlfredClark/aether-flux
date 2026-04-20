mod loader;
mod qwen3_asr;
mod sense_voice_small;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::{async_runtime, AppHandle, State};

use self::{
    loader::{resolve_model_root, DynAsrLoader, ExecutionMode},
    qwen3_asr::{Qwen3AsrLoader, Qwen3AsrLoaderConfig},
    sense_voice_small::{SenseVoiceSmallLoader, SenseVoiceSmallLoaderConfig},
};

/// ASR 模型类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrModelKind {
    /// Qwen3-ASR
    Qwen3Asr,
    /// SenseVoiceSmall
    #[serde(alias = "sensevoice_small")]
    SenseVoiceSmall,
}

/// ASR 运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrExecutionMode {
    /// 优先使用GPU，GPU无效则回滚到CPU
    Auto,
    /// 只使用CPU推理
    OnlyCpu,
}

/// ASR 运行时
#[derive(Default)]
pub struct AsrState {
    /// 当前已加载的 ASR 运行时，放在 Mutex 中以便命令串行访问和安全替换。
    pub(crate) inner: Arc<Mutex<AsrStateInner>>,
}

/// ASR 运行时内部
#[derive(Default)]
pub(crate) struct AsrStateInner {
    pub(crate) current_model: Option<AsrModelKind>,
    pub(crate) current_mode: Option<AsrExecutionMode>,
    pub(crate) runtime: Option<DynAsrLoader>,
}

/// ASR 状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStatus {
    pub is_loaded: bool,
    pub current_model: Option<AsrModelKind>,
    pub current_mode: Option<AsrExecutionMode>,
}

impl AsrStatus {
    /// 根据内部状态快照构造前端可消费的状态对象。
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

/// 查询当前 ASR 运行时的加载状态。
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
        let text = runtime
            .recognize_wav_text(&wav_path)
            .map_err(|err| format_recognition_error(model, err))?;

        Ok(AsrRecognitionResult {
            text,
            model,
            audio_path: wav_path.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|err| format!("Failed to join ASR recognition task: {err}"))?
}

/// 根据模型类型和执行模式创建对应的 ASR 运行时。
fn build_runtime(
    app: &AppHandle,
    model: AsrModelKind,
    mode: AsrExecutionMode,
) -> Result<DynAsrLoader, String> {
    match model {
        AsrModelKind::Qwen3Asr => {
            let root = resolve_model_root(app, "Qwen3-ASR-onnx", |root| {
                root.join("model_0.6B").is_dir() && root.join("tokenizer").is_dir()
            })?;
            let mut config = Qwen3AsrLoaderConfig::from_root(root, "model_0.6B");
            config.runtime.execution_mode = map_execution_mode(mode);
            let loader = Qwen3AsrLoader::new(config)
                .map_err(|err| format!("failed to load Qwen3-ASR: {err:#}"))?;
            Ok(Box::new(loader))
        }
        AsrModelKind::SenseVoiceSmall => {
            let root = resolve_model_root(app, "SenseVoiceSmall-onnx", |root| {
                root.join("model_quant.onnx").is_file()
                    && root.join("tokens.json").is_file()
                    && root.join("am.mvn").is_file()
            })?;
            let mut config = SenseVoiceSmallLoaderConfig::from_root(root);
            config.runtime.execution_mode = map_execution_mode(mode);
            let loader = SenseVoiceSmallLoader::new(config)
                .map_err(|err| format!("failed to load SenseVoiceSmall: {err:#}"))?;
            Ok(Box::new(loader))
        }
    }
}

/// 将对外暴露的执行模式映射为底层 ORT 执行模式。
fn map_execution_mode(mode: AsrExecutionMode) -> ExecutionMode {
    match mode {
        AsrExecutionMode::Auto => ExecutionMode::Auto,
        AsrExecutionMode::OnlyCpu => ExecutionMode::CpuOnly,
    }
}

/// 根据模型类型格式化统一的识别错误信息。
fn format_recognition_error(model: AsrModelKind, err: anyhow::Error) -> String {
    match model {
        AsrModelKind::Qwen3Asr => format!("Qwen3-ASR recognition failed: {err:#}"),
        AsrModelKind::SenseVoiceSmall => {
            format!("SenseVoiceSmall recognition failed: {err:#}")
        }
    }
}
