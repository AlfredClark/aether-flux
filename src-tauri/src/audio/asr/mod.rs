mod loader;
pub mod processor;
mod qwen3_asr;
mod sense_voice_small;
pub mod wordbank;

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tauri::{async_runtime, AppHandle, Manager, State};

use self::{
    loader::{resolve_model_root, DynAsrLoader, ExecutionMode},
    processor::{JiebaDecomposer, WordbankFitter, WordbankFitterReplacement},
    qwen3_asr::{Qwen3AsrLoader, Qwen3AsrLoaderConfig},
    sense_voice_small::{SenseVoiceSmallLoader, SenseVoiceSmallLoaderConfig},
    wordbank::{
        collect_enabled_wordbank_fitter_entries, collect_enabled_wordbank_words, WordbankState,
    },
};
use crate::utils::backend_i18n::{localize_error, tr, tr_args};

#[macro_export]
macro_rules! asr_commands {
    ($callback:ident [$($acc:path,)*] $($rest:ident)*) => {
        $crate::wordbank_commands!(
            $callback
            [
                $($acc,)*
                $crate::audio::asr::get_asr_status,
                $crate::audio::asr::get_asr_recording_cache_stats,
                $crate::audio::asr::clear_asr_recording_cache,
                $crate::audio::asr::rebuild_asr_fitter,
                $crate::audio::asr::rebuild_asr_decomposer,
                $crate::audio::asr::load_asr_model,
                $crate::audio::asr::destroy_asr_model,
                $crate::audio::asr::recognize_audio,
            ]
            $($rest)*
        )
    };
}

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

/// ASR 识别语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrLanguage {
    /// 自动检测语言
    Auto,
    /// 简体中文
    Zh,
    /// 英文
    En,
    /// 粤语
    Yue,
    /// 日语
    Ja,
    /// 韩语
    Ko,
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
    pub(crate) current_language: Option<AsrLanguage>,
    pub(crate) runtime: Option<DynAsrLoader>,
    pub(crate) fitter: Option<WordbankFitter>,
    pub(crate) decomposer: Option<JiebaDecomposer>,
}

/// ASR 状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStatus {
    pub is_loaded: bool,
    pub current_model: Option<AsrModelKind>,
    pub current_mode: Option<AsrExecutionMode>,
    pub current_language: Option<AsrLanguage>,
}

impl AsrStatus {
    /// 根据内部状态快照构造前端可消费的状态对象。
    fn from_inner(inner: &AsrStateInner) -> Self {
        Self {
            is_loaded: inner.runtime.is_some(),
            current_model: inner.current_model,
            current_mode: inner.current_mode,
            current_language: inner.current_language,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrRecognitionResult {
    pub text: AsrRecognitionText,
    pub model: AsrModelKind,
    pub audio_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AsrRecognitionText {
    Plain(String),
    Segmented(Vec<String>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrRecordingCacheStats {
    pub file_count: usize,
    pub total_bytes: u64,
}

/// 查询当前 ASR 运行时的加载状态。
#[tauri::command]
pub fn get_asr_status(app: AppHandle, asr_state: State<'_, AsrState>) -> Result<AsrStatus, String> {
    let guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(&app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    Ok(AsrStatus::from_inner(&guard))
}

/// 页面加载或用户切换模型时调用，旧模型会在替换时自动释放。
#[tauri::command]
pub async fn load_asr_model(
    model: AsrModelKind,
    mode: AsrExecutionMode,
    language: AsrLanguage,
    app: AppHandle,
    asr_state: State<'_, AsrState>,
    wordbank_state: State<'_, WordbankState>,
) -> Result<AsrStatus, String> {
    let app_for_runtime = app.clone();
    let state = Arc::clone(&asr_state.inner);
    let runtime = async_runtime::spawn_blocking(move || {
        build_runtime(&app_for_runtime, model, mode, language)
    })
    .await
    .map_err(|err| {
        tr_args(
            &app,
            "backend.asr.join_loader_task_failed",
            "Failed to join ASR loader task: {err}",
            &[("err", err.to_string())],
        )
    })??;

    let mut guard = state
        .lock()
        .map_err(|_| tr(&app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    guard.current_model = Some(model);
    guard.current_mode = Some(mode);
    guard.current_language = Some(language);
    guard.runtime = Some(runtime);
    drop(guard);
    reinitialize_asr_fitter(&app, &wordbank_state, &asr_state)?;
    reinitialize_asr_decomposer(&app, &wordbank_state, &asr_state)?;

    let guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(&app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    Ok(AsrStatus::from_inner(&guard))
}

/// 页面退出时主动销毁模型，避免 ONNX Session 长时间占用内存和显存。
#[tauri::command]
pub fn destroy_asr_model(app: AppHandle, asr_state: State<'_, AsrState>) -> Result<(), String> {
    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(&app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    guard.current_model = None;
    guard.current_mode = None;
    guard.current_language = None;
    guard.runtime = None;
    guard.fitter = None;
    guard.decomposer = None;
    Ok(())
}

#[tauri::command]
pub fn get_asr_recording_cache_stats(app: AppHandle) -> Result<AsrRecordingCacheStats, String> {
    let cache_dir = resolve_recorder_cache_dir(&app).map_err(|err| {
        localize_error(
            &app,
            &format!("failed to resolve recorder cache dir: {err:#}"),
        )
    })?;

    collect_wav_cache_stats(&cache_dir)
        .map_err(|err| localize_error(&app, &format!("failed to collect cache stats: {err:#}")))
}

#[tauri::command]
pub fn clear_asr_recording_cache(app: AppHandle) -> Result<AsrRecordingCacheStats, String> {
    let cache_dir = resolve_recorder_cache_dir(&app).map_err(|err| {
        localize_error(
            &app,
            &format!("failed to resolve recorder cache dir: {err:#}"),
        )
    })?;

    clear_wav_cache_files(&cache_dir).map_err(|err| {
        localize_error(&app, &format!("failed to clear recording cache: {err:#}"))
    })?;
    collect_wav_cache_stats(&cache_dir)
        .map_err(|err| localize_error(&app, &format!("failed to collect cache stats: {err:#}")))
}

#[tauri::command]
pub fn rebuild_asr_decomposer(
    app: AppHandle,
    asr_state: State<'_, AsrState>,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    reinitialize_asr_decomposer(&app, &wordbank_state, &asr_state)
}

#[tauri::command]
pub fn rebuild_asr_fitter(
    app: AppHandle,
    asr_state: State<'_, AsrState>,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    reinitialize_asr_fitter(&app, &wordbank_state, &asr_state)
}

/// 停止录音后立即调用该命令执行离线识别，并把文本结果回传给前端页面。
#[tauri::command]
pub async fn recognize_audio(
    wav_path: String,
    enable_fitting: bool,
    enable_decomposition: bool,
    app: AppHandle,
    asr_state: State<'_, AsrState>,
) -> Result<AsrRecognitionResult, String> {
    let state = Arc::clone(&asr_state.inner);
    let result: Result<AsrRecognitionResult, String> = async_runtime::spawn_blocking(move || {
        let wav_path = PathBuf::from(wav_path);
        let mut guard = state
            .lock()
            .map_err(|_| "Failed to lock ASR state".to_string())?;
        let model = guard
            .current_model
            .ok_or_else(|| "ASR model is not initialized".to_string())?;
        let language = guard
            .current_language
            .ok_or_else(|| "ASR language is not initialized".to_string())?;
        let runtime = guard
            .runtime
            .as_mut()
            .ok_or_else(|| "ASR model is not loaded".to_string())?;
        let text = runtime
            .recognize_wav_text(&wav_path)
            .map_err(|err| format_recognition_error(model, err))?;
        let text = maybe_fit_text(&guard, language, enable_fitting, text)?;
        let text = maybe_decompose_text(&guard, language, enable_decomposition, text)?;

        Ok(AsrRecognitionResult {
            text,
            model,
            audio_path: wav_path.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|err| {
        tr_args(
            &app,
            "backend.asr.join_recognition_task_failed",
            "Failed to join ASR recognition task: {err}",
            &[("err", err.to_string())],
        )
    })?;
    result.map_err(|err| localize_error(&app, &err))
}

/// 在启用词库拟合且语言允许的前提下，对识别文本执行拼音词库拟合。
fn maybe_fit_text(
    inner: &AsrStateInner,
    language: AsrLanguage,
    enable_fitting: bool,
    text: String,
) -> Result<String, String> {
    if !enable_fitting || !supports_decomposition(language) || text.trim().is_empty() {
        return Ok(text);
    }

    let fitter = inner
        .fitter
        .as_ref()
        .ok_or_else(|| "ASR fitter is not loaded".to_string())?;
    fitter
        .fit(&text)
        .map_err(|err| format!("ASR fitting failed: {err:#}"))
}

/// 在启用分词且语言允许的前提下，对识别文本执行 jieba 分词。
fn maybe_decompose_text(
    inner: &AsrStateInner,
    language: AsrLanguage,
    enable_decomposition: bool,
    text: String,
) -> Result<AsrRecognitionText, String> {
    if !enable_decomposition || !supports_decomposition(language) || text.trim().is_empty() {
        return Ok(AsrRecognitionText::Plain(text));
    }

    let decomposer = inner
        .decomposer
        .as_ref()
        .ok_or_else(|| "ASR decomposer is not loaded".to_string())?;
    let segments = decomposer
        .decompose(&text)
        .map_err(|err| format!("ASR decomposition failed: {err:#}"))?;

    if segments.is_empty() {
        Ok(AsrRecognitionText::Plain(text))
    } else {
        Ok(AsrRecognitionText::Segmented(segments))
    }
}

pub(crate) fn reinitialize_asr_decomposer(
    app: &AppHandle,
    wordbank_state: &WordbankState,
    asr_state: &AsrState,
) -> Result<(), String> {
    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    if guard.runtime.is_none() {
        guard.decomposer = None;
        return Err(tr(
            app,
            "backend.asr.model_not_loaded",
            "ASR model is not loaded",
        ));
    }
    drop(guard);
    let enabled_words = collect_enabled_wordbank_words(app, wordbank_state)?;
    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    guard.decomposer = Some(JiebaDecomposer::new(enabled_words));
    Ok(())
}

pub(crate) fn reinitialize_asr_fitter(
    app: &AppHandle,
    wordbank_state: &WordbankState,
    asr_state: &AsrState,
) -> Result<(), String> {
    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    if guard.runtime.is_none() {
        guard.fitter = None;
        return Err(tr(
            app,
            "backend.asr.model_not_loaded",
            "ASR model is not loaded",
        ));
    }
    drop(guard);

    let fitter_entries = collect_enabled_wordbank_fitter_entries(app, wordbank_state)?;
    let preferred_words = fitter_entries
        .into_iter()
        .map(|entry| WordbankFitterReplacement {
            key: entry.key,
            value: entry.value,
            prefix: entry.prefix,
            suffix: entry.suffix,
        })
        .collect::<Vec<_>>();

    let mut guard = asr_state
        .inner
        .lock()
        .map_err(|_| tr(app, "backend.asr.lock_failed", "Failed to lock ASR state"))?;
    guard.fitter = Some(WordbankFitter::new(preferred_words));
    Ok(())
}

/// 根据模型类型和执行模式创建对应的 ASR 运行时。
fn build_runtime(
    app: &AppHandle,
    model: AsrModelKind,
    mode: AsrExecutionMode,
    language: AsrLanguage,
) -> Result<DynAsrLoader, String> {
    match model {
        AsrModelKind::Qwen3Asr => {
            let root = resolve_model_root(app, "Qwen3-ASR-onnx", |root| {
                root.join("model_0.6B").is_dir() && root.join("tokenizer").is_dir()
            })?;
            let mut config = Qwen3AsrLoaderConfig::from_root(root, "model_0.6B");
            config.runtime.execution_mode = map_execution_mode(mode);
            config.language = Some(map_qwen3_language(language).to_string());
            let loader = Qwen3AsrLoader::new(config).map_err(|err| {
                localize_error(app, &format!("failed to load Qwen3-ASR: {err:#}"))
            })?;
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
            config.language_token = map_sensevoice_language(language).to_string();
            let loader = SenseVoiceSmallLoader::new(config).map_err(|err| {
                localize_error(app, &format!("failed to load SenseVoiceSmall: {err:#}"))
            })?;
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

fn resolve_recorder_cache_dir(app: &AppHandle) -> Result<PathBuf, anyhow::Error> {
    Ok(app
        .path()
        .app_cache_dir()
        .context("app_cache_dir was not available")?
        .join("recorder"))
}

fn collect_wav_cache_stats(cache_dir: &PathBuf) -> Result<AsrRecordingCacheStats, anyhow::Error> {
    if !cache_dir.exists() {
        return Ok(AsrRecordingCacheStats {
            file_count: 0,
            total_bytes: 0,
        });
    }

    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    let mut stack = vec![cache_dir.clone()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                file_count += 1;
                total_bytes += entry.metadata()?.len();
            }
        }
    }

    Ok(AsrRecordingCacheStats {
        file_count,
        total_bytes,
    })
}

fn clear_wav_cache_files(cache_dir: &PathBuf) -> Result<(), anyhow::Error> {
    if !cache_dir.exists() {
        return Ok(());
    }

    let mut stack = vec![cache_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                fs::remove_file(&path)?;
            }
        }
    }

    Ok(())
}

/// 将统一语言枚举映射为 Qwen3-ASR prompt 里使用的语言字符串。
fn map_qwen3_language(language: AsrLanguage) -> &'static str {
    match language {
        AsrLanguage::Auto => "auto",
        AsrLanguage::Zh => "Chinese",
        AsrLanguage::En => "English",
        AsrLanguage::Yue => "Cantonese",
        AsrLanguage::Ja => "Japanese",
        AsrLanguage::Ko => "Korean",
    }
}

/// 将统一语言枚举映射为 SenseVoiceSmall 需要的语言 token。
fn map_sensevoice_language(language: AsrLanguage) -> &'static str {
    match language {
        AsrLanguage::Auto => "<|auto|>",
        AsrLanguage::Zh => "<|zh|>",
        AsrLanguage::En => "<|en|>",
        AsrLanguage::Yue => "<|yue|>",
        AsrLanguage::Ja => "<|ja|>",
        AsrLanguage::Ko => "<|ko|>",
    }
}

/// 仅在中文或自动语言时允许启用分词。
fn supports_decomposition(language: AsrLanguage) -> bool {
    matches!(language, AsrLanguage::Auto | AsrLanguage::Zh)
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
