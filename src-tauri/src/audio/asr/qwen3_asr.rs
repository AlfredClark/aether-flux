use std::{
    collections::{BTreeSet, HashMap},
    f32::consts::PI,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use ndarray::{Array2, Array3, Array4, Axis, Ix3, Ix4, s};
use ort::{
    ep::{self, ExecutionProvider},
    session::{
        Session,
        builder::{GraphOptimizationLevel, SessionBuilder},
    },
    value::TensorRef,
};
use realfft::RealFftPlanner;
use serde::Deserialize;
use tokenizers::{
    AddedToken, Tokenizer,
    decoders::byte_level::ByteLevel as ByteLevelDecoder,
    models::bpe::BPE,
    pre_tokenizers::byte_level::ByteLevel,
};

const DEFAULT_SAMPLE_RATE: usize = 16_000;

/// 推理设备选择策略。
///
/// `Auto` 会优先尝试 CUDA，不可用时自动回退到 CPU。
/// `CpuOnly` 则完全跳过 GPU 探测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Auto,
    CpuOnly,
}

#[derive(Debug, Clone)]
/// ASR 加载器的初始化配置。
pub struct Qwen3AsrLoaderConfig {
    pub model_dir: PathBuf,
    pub tokenizer_dir: PathBuf,
    pub config_json: PathBuf,
    pub preprocessor_config_json: PathBuf,
    pub tokenizer_config_json: PathBuf,
    pub max_new_tokens: usize,
    pub language: Option<String>,
    pub intra_threads: usize,
    pub inter_threads: usize,
    pub optimization_level: GraphOptimizationLevel,
    pub execution_mode: ExecutionMode,
    pub gpu_device_id: i32,
}

impl Qwen3AsrLoaderConfig {
    /// 根据 `Qwen3-ASR-onnx` 根目录生成默认配置。
    pub fn from_root(root_dir: impl AsRef<Path>, model_subdir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let model_dir = root_dir.join(model_subdir);
        let tokenizer_dir = root_dir.join("tokenizer");
        Self {
            model_dir,
            tokenizer_dir,
            config_json: root_dir.join("tokenizer/config.json"),
            preprocessor_config_json: root_dir.join("tokenizer/preprocessor_config.json"),
            tokenizer_config_json: root_dir.join("tokenizer/tokenizer_config.json"),
            max_new_tokens: 256,
            language: None,
            intra_threads: 1,
            inter_threads: 1,
            optimization_level: GraphOptimizationLevel::Level3,
            execution_mode: ExecutionMode::Auto,
            gpu_device_id: 0,
        }
    }
}

/// 面向 `Qwen3-ASR-onnx` 的高层加载器封装。
pub struct Qwen3AsrLoader {
    config: Qwen3AsrLoaderConfig,
    tokenizer: Tokenizer,
    feature_extractor: WhisperFeatureExtractor,
    conv_frontend: Session,
    encoder: Session,
    decoder: Session,
    audio_start_token_id: u32,
    audio_end_token_id: u32,
    audio_pad_token_id: u32,
    stop_token_ids: BTreeSet<u32>,
    decoder_layers: usize,
    kv_heads: usize,
    head_dim: usize,
}

impl Qwen3AsrLoader {
    /// 加载 tokenizer、特征提取器和三个 ONNX 子模型。
    pub fn new(config: Qwen3AsrLoaderConfig) -> Result<Self> {
        let model_config: ModelConfigFile = read_json(&config.config_json)?;
        let preprocessor_config: PreprocessorConfigFile = read_json(&config.preprocessor_config_json)?;
        let tokenizer_config: TokenizerConfigFile = read_json(&config.tokenizer_config_json)?;

        let tokenizer = build_tokenizer(&config.tokenizer_dir, &tokenizer_config)?;
        let mut stop_token_ids = BTreeSet::new();
        if let Some(token) = tokenizer_config.eos_token.as_deref() {
            if let Some(id) = tokenizer.token_to_id(token) {
                stop_token_ids.insert(id);
            }
        }
        if let Some(token) = tokenizer_config.pad_token.as_deref() {
            if let Some(id) = tokenizer.token_to_id(token) {
                stop_token_ids.insert(id);
            }
        }
        if let Some(id) = tokenizer.token_to_id("<|im_end|>") {
            stop_token_ids.insert(id);
        }
        for token in [
            "<|fim_prefix|>",
            "<|fim_middle|>",
            "<|fim_suffix|>",
            "<tool_call>",
            "</tool_call>",
            "<tool_response>",
            "</tool_response>",
            "<think>",
            "</think>",
        ] {
            if let Some(id) = tokenizer.token_to_id(token) {
                stop_token_ids.insert(id);
            }
        }

        let conv_frontend = build_session(&config, config.model_dir.join("conv_frontend.onnx"))?;
        let encoder = build_session(&config, config.model_dir.join("encoder.onnx"))?;
        let decoder = build_session(&config, config.model_dir.join("decoder.onnx"))?;

        let (decoder_layers, kv_heads, head_dim) = infer_decoder_cache_layout(&decoder)?;

        Ok(Self {
            config,
            tokenizer,
            feature_extractor: WhisperFeatureExtractor::new(preprocessor_config),
            conv_frontend,
            encoder,
            decoder,
            audio_start_token_id: model_config.thinker_config.audio_start_token_id,
            audio_end_token_id: model_config.thinker_config.audio_end_token_id,
            audio_pad_token_id: model_config.thinker_config.audio_token_id,
            stop_token_ids,
            decoder_layers,
            kv_heads,
            head_dim,
        })
    }

    /// 直接接收 wav 文件路径并返回识别文本。
    pub fn recognize_wav(&mut self, wav_path: impl AsRef<Path>) -> Result<String> {
        let samples = load_wav_mono(wav_path.as_ref(), self.feature_extractor.sample_rate)?;
        let input_features = self.feature_extractor.extract(&samples)?;
        let conv_output = self.run_conv_frontend(&input_features)?;
        let audio_features = self.run_encoder(&conv_output)?;
        let prompt_ids = self.build_prompt(audio_features.len_of(Axis(1)))?;
        let generated_ids = self.generate(&audio_features, &prompt_ids)?;
        let text = self
            .tokenizer
            .decode(&generated_ids, true)
            .map_err(tokenizer_error)?;

        let cleaned = clean_asr_response(&text);
        if cleaned.is_empty() {
            Ok(text.trim().to_string())
        } else {
            Ok(cleaned)
        }
    }

    /// 运行卷积前端，将 log-mel 特征压缩成 encoder 需要的表示。
    fn run_conv_frontend(&mut self, input_features: &Array3<f32>) -> Result<Array3<f32>> {
        let outputs = self.conv_frontend.run(ort::inputs! {
            "input_features" => TensorRef::from_array_view(input_features.view())?
        })?;
        let array = outputs["conv_output"]
            .try_extract_array::<f32>()
            .context("failed to extract conv_frontend output")?
            .into_dimensionality::<Ix3>()
            .context("conv_frontend output did not have rank 3")?;
        Ok(array.to_owned())
    }

    /// 运行 encoder，得到 decoder 消费的音频特征序列。
    fn run_encoder(&mut self, conv_output: &Array3<f32>) -> Result<Array3<f32>> {
        let n_audio_tokens = conv_output.len_of(Axis(1));
        let feature_attention_mask = Array2::<bool>::from_elem((1, n_audio_tokens), true);
        let outputs = self.encoder.run(ort::inputs! {
            "input_features" => TensorRef::from_array_view(conv_output.view())?,
            "feature_attention_mask" => TensorRef::from_array_view(feature_attention_mask.view())?
        })?;
        let array = outputs["audio_features"]
            .try_extract_array::<f32>()
            .context("failed to extract encoder output")?
            .into_dimensionality::<Ix3>()
            .context("encoder output did not have rank 3")?;
        Ok(array.to_owned())
    }

    /// 构造 Qwen3-ASR 需要的 prompt。
    ///
    /// Qwen3-ASR 需要标准聊天模板：
    /// system -> user(audio) -> assistant
    ///
    /// 仅传递音频 token 会导致模型缺少对话上下文，常见现象就是能够跑通推理但输出为空。
    fn build_prompt(&self, n_audio_tokens: usize) -> Result<Vec<i64>> {
        let target_language = self.config.language.as_deref().unwrap_or("Chinese");
        let prefix = self
            .tokenizer
            .encode("<|im_start|>system\n<|im_end|>\n<|im_start|>user\n", false)
            .map_err(tokenizer_error)
            .context("failed to encode Qwen3-ASR prompt prefix")?;
        let suffix = self
            .tokenizer
            .encode(
                format!(
                    "<|im_end|>\n<|im_start|>assistant\n language {target_language}<asr_text>"
                ),
                false,
            )
            .map_err(tokenizer_error)
            .context("failed to encode Qwen3-ASR prompt suffix")?;

        let mut prompt = Vec::with_capacity(prefix.len() + n_audio_tokens + suffix.len() + 16);
        prompt.extend(prefix.get_ids().iter().map(|&id| i64::from(id)));
        prompt.push(self.audio_start_token_id as i64);
        prompt.extend(std::iter::repeat(self.audio_pad_token_id as i64).take(n_audio_tokens));
        prompt.push(self.audio_end_token_id as i64);
        prompt.extend(suffix.get_ids().iter().map(|&id| i64::from(id)));
        Ok(prompt)
    }

    /// 自回归生成识别结果，并维护 decoder KV cache。
    fn generate(&mut self, audio_features: &Array3<f32>, prompt_ids: &[i64]) -> Result<Vec<u32>> {
        let mut cache_keys = (0..self.decoder_layers)
            .map(|_| Array4::<f32>::zeros((1, prompt_ids.len() + self.config.max_new_tokens, self.kv_heads, self.head_dim)))
            .collect::<Vec<_>>();
        let mut cache_values = (0..self.decoder_layers)
            .map(|_| Array4::<f32>::zeros((1, prompt_ids.len() + self.config.max_new_tokens, self.kv_heads, self.head_dim)))
            .collect::<Vec<_>>();

        let mut generated_ids = Vec::new();
        let mut current_input_ids = prompt_ids.to_vec();
        let mut attention_mask = vec![1_i64; prompt_ids.len()];
        let mut cache_position = (0..prompt_ids.len())
            .map(|idx| idx as i64)
            .collect::<Vec<_>>();

        for _ in 0..self.config.max_new_tokens {
            let mut inputs = ort::inputs! {
                "input_ids" => TensorRef::from_array_view(([1_usize, current_input_ids.len()], current_input_ids.as_slice()))?,
                "audio_features" => TensorRef::from_array_view(audio_features.view())?,
                "attention_mask" => TensorRef::from_array_view(([1_usize, attention_mask.len()], attention_mask.as_slice()))?,
                "cache_position" => TensorRef::from_array_view(([cache_position.len()], cache_position.as_slice()))?
            };

            for layer_idx in 0..self.decoder_layers {
                inputs.push((
                    format!("cache_key_{layer_idx}").into(),
                    TensorRef::from_array_view(cache_keys[layer_idx].view())?.into(),
                ));
                inputs.push((
                    format!("cache_value_{layer_idx}").into(),
                    TensorRef::from_array_view(cache_values[layer_idx].view())?.into(),
                ));
            }

            let outputs = self.decoder.run(inputs)?;
            Self::update_caches(
                self.decoder_layers,
                &outputs,
                &cache_position,
                &mut cache_keys,
                &mut cache_values,
            )?;

            let logits = outputs["logits"]
                .try_extract_array::<f32>()
                .context("failed to extract decoder logits")?
                .into_dimensionality::<Ix3>()
                .context("decoder logits did not have rank 3")?;
            let next_token_id = argmax(logits.slice(s![0, current_input_ids.len() - 1, ..])) as u32;

            if self.stop_token_ids.contains(&next_token_id) {
                break;
            }

            generated_ids.push(next_token_id);
            current_input_ids.clear();
            current_input_ids.push(i64::from(next_token_id));
            attention_mask.push(1);
            cache_position.clear();
            cache_position.push((prompt_ids.len() + generated_ids.len() - 1) as i64);
        }

        Ok(generated_ids)
    }

    /// 将每一步 decoder 产生的增量 KV 写回完整缓存。
    fn update_caches(
        decoder_layers: usize,
        outputs: &ort::session::SessionOutputs<'_>,
        cache_position: &[i64],
        cache_keys: &mut [Array4<f32>],
        cache_values: &mut [Array4<f32>],
    ) -> Result<()> {
        let start = *cache_position
            .first()
            .ok_or_else(|| anyhow!("cache_position cannot be empty"))? as usize;
        for (offset, position) in cache_position.iter().enumerate() {
            ensure!(
                *position as usize == start + offset,
                "cache_position must be contiguous, got {cache_position:?}"
            );
        }
        let end = start + cache_position.len();

        for layer_idx in 0..decoder_layers {
            let key_delta = outputs[format!("key_delta_{layer_idx}")]
                .try_extract_array::<f32>()
                .with_context(|| format!("failed to extract key_delta_{layer_idx}"))?
                .into_dimensionality::<Ix4>()
                .with_context(|| format!("key_delta_{layer_idx} did not have rank 4"))?;
            let value_delta = outputs[format!("value_delta_{layer_idx}")]
                .try_extract_array::<f32>()
                .with_context(|| format!("failed to extract value_delta_{layer_idx}"))?
                .into_dimensionality::<Ix4>()
                .with_context(|| format!("value_delta_{layer_idx} did not have rank 4"))?;

            cache_keys[layer_idx]
                .slice_mut(s![.., start..end, .., ..])
                .assign(&key_delta);
            cache_values[layer_idx]
                .slice_mut(s![.., start..end, .., ..])
                .assign(&value_delta);
        }

        Ok(())
    }
}

/// 创建 ONNX Session，并根据配置决定是否启用 GPU。
///
/// 当 CUDA 不可用时，这里会保持默认 CPU 路径，不让初始化失败。
fn build_session(config: &Qwen3AsrLoaderConfig, path: impl AsRef<Path>) -> Result<Session> {
    let path = path.as_ref();
    let builder = Session::builder().map_err(ort_error)?;
    let builder = builder
        .with_optimization_level(config.optimization_level)
        .map_err(ort_builder_error)?;
    let builder = builder
        .with_intra_threads(config.intra_threads)
        .map_err(ort_builder_error)?;
    let builder = builder
        .with_inter_threads(config.inter_threads)
        .map_err(ort_builder_error)?;
    let builder = builder.with_memory_pattern(false).map_err(ort_builder_error)?;
    let mut builder = configure_execution_providers(builder, config)?;
    let session = builder
        .commit_from_file(path)
        .map_err(ort_error)
        .with_context(|| format!("failed to load ONNX model from {}", path.display()))?;
    Ok(session)
}

/// 为当前 SessionBuilder 配置执行提供者。
///
/// `Auto` 模式会优先尝试 CUDA。只要当前 ORT 构建或宿主环境不支持 CUDA，
/// 就自动回退为默认 CPU 执行路径。
fn configure_execution_providers(
    builder: SessionBuilder,
    config: &Qwen3AsrLoaderConfig,
) -> Result<SessionBuilder> {
    match config.execution_mode {
        ExecutionMode::CpuOnly => Ok(builder),
        ExecutionMode::Auto => {
            let cuda = ep::CUDA::default().with_device_id(config.gpu_device_id);
            match cuda.is_available() {
                Ok(true) => println!("CUDA available"),
                Ok(false) | Err(_) => println!("CUDA unavailable"),
            }
            match cuda.is_available() {
                Ok(true) => builder
                    .with_execution_providers([cuda.build()])
                    .map_err(ort_builder_error),
                Ok(false) | Err(_) => Ok(builder),
            }
        }
    }
}

/// 从 decoder 输入张量的形状中推断 KV cache 的布局。
fn infer_decoder_cache_layout(decoder: &Session) -> Result<(usize, usize, usize)> {
    let mut layer_count = 0usize;
    let mut kv_heads = None;
    let mut head_dim = None;

    for input in decoder.inputs() {
        let name = input.name();
        if !name.starts_with("cache_key_") {
            continue;
        }

        let shape = input
            .dtype()
            .tensor_shape()
            .ok_or_else(|| anyhow!("decoder cache input {name} was not a tensor"))?;
        ensure!(shape.len() == 4, "decoder cache input {name} must have rank 4, got {shape:?}");
        let shape_values = shape.iter().copied().collect::<Vec<_>>();
        let current_kv_heads = usize::try_from(shape_values[2]).context("cache kv_heads was negative")?;
        let current_head_dim = usize::try_from(shape_values[3]).context("cache head_dim was negative")?;
        kv_heads.get_or_insert(current_kv_heads);
        head_dim.get_or_insert(current_head_dim);
        ensure!(
            kv_heads == Some(current_kv_heads) && head_dim == Some(current_head_dim),
            "decoder cache shapes were inconsistent"
        );
        layer_count += 1;
    }

    ensure!(layer_count > 0, "decoder did not expose any cache inputs");

    Ok((
        layer_count,
        kv_heads.ok_or_else(|| anyhow!("missing decoder kv_heads"))?,
        head_dim.ok_or_else(|| anyhow!("missing decoder head_dim"))?,
    ))
}

/// 读取并反序列化 JSON 配置文件。
fn read_json<T: for<'de> Deserialize<'de>>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file)).with_context(|| format!("failed to parse {}", path.display()))
}

/// 使用导出的 tokenizer 文件重建 Rust 侧 tokenizer。
fn build_tokenizer(tokenizer_dir: &Path, tokenizer_config: &TokenizerConfigFile) -> Result<Tokenizer> {
    let vocab_path = tokenizer_dir.join("vocab.json");
    let merges_path = tokenizer_dir.join("merges.txt");
    let vocab_path_str = vocab_path
        .to_str()
        .ok_or_else(|| anyhow!("invalid UTF-8 path: {}", vocab_path.display()))?;
    let merges_path_str = merges_path
        .to_str()
        .ok_or_else(|| anyhow!("invalid UTF-8 path: {}", merges_path.display()))?;

    let bpe = BPE::from_file(vocab_path_str, merges_path_str)
        .build()
        .map_err(tokenizer_error)
        .context("failed to build BPE tokenizer")?;
    let mut tokenizer = Tokenizer::new(bpe);
    let byte_level = ByteLevel::new(false, false, true);
    tokenizer.with_pre_tokenizer(Some(byte_level));
    tokenizer.with_decoder(Some(ByteLevelDecoder::default()));
    tokenizer.set_encode_special_tokens(true);

    let mut regular_tokens = Vec::new();
    let mut special_tokens = Vec::new();
    let mut entries = tokenizer_config
        .added_tokens_decoder
        .iter()
        .collect::<Vec<_>>();
    entries.sort_by_key(|(token_id, _)| token_id.parse::<u32>().unwrap_or_default());

    for (_, token) in entries {
        let added_token = AddedToken::from(token.content.clone(), token.special)
            .single_word(token.single_word)
            .lstrip(token.lstrip)
            .rstrip(token.rstrip)
            .normalized(token.normalized)
            .special(token.special);
        if token.special {
            special_tokens.push(added_token);
        } else {
            regular_tokens.push(added_token);
        }
    }

    tokenizer.add_tokens(&regular_tokens);
    tokenizer.add_special_tokens(&special_tokens);
    Ok(tokenizer)
}

fn ort_error(error: ort::Error) -> anyhow::Error {
    anyhow!(error.to_string())
}

fn ort_builder_error(error: ort::Error<ort::session::builder::SessionBuilder>) -> anyhow::Error {
    anyhow!(error.to_string())
}

fn tokenizer_error(error: Box<dyn std::error::Error + Send + Sync>) -> anyhow::Error {
    anyhow!(error.to_string())
}

/// 读取 wav 并转换成单声道 `f32` PCM。
///
/// 如果输入是多声道，这里按帧求平均；采样率不符合预期时直接报错。
fn load_wav_mono(path: &Path, expected_sample_rate: usize) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open wav file {}", path.display()))?;
    let spec = reader.spec();
    ensure!(
        spec.sample_rate as usize == expected_sample_rate,
        "expected {expected_sample_rate} Hz wav, got {} Hz for {}",
        spec.sample_rate,
        path.display()
    );
    ensure!(spec.channels >= 1, "wav file had zero channels: {}", path.display());

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
        let avg = frame.iter().sum::<f32>() / channels as f32;
        mono.push(avg);
    }
    Ok(mono)
}

/// 在贪心解码时返回最大值对应的下标。
fn argmax(values: ndarray::ArrayView1<'_, f32>) -> usize {
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

/// 清理模板 token，只保留最终识别文本。
fn clean_asr_response(raw: &str) -> String {
    let cleaned = if let Some(start) = raw.find("<asr_text>") {
        raw[start + "<asr_text>".len()..].trim()
    } else {
        raw.trim()
    };
    cleaned.trim_matches(|ch| ch == '\'' || ch == '"').trim().to_string()
}

/// Whisper 风格的音频特征提取器。
///
/// Qwen3-ASR-onnx 的输入前处理与 Whisper 类似，这里在 Rust 侧直接复现
/// log-mel 特征计算逻辑，避免额外依赖 Python。
struct WhisperFeatureExtractor {
    feature_size: usize,
    n_fft: usize,
    hop_length: usize,
    sample_rate: usize,
    hann_window: Vec<f32>,
    mel_filters: Array2<f32>,
    rfft: Arc<dyn realfft::RealToComplex<f32>>,
}

impl WhisperFeatureExtractor {
    /// 根据导出的预处理配置初始化 FFT、窗函数和 mel filter。
    fn new(config: PreprocessorConfigFile) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let rfft = planner.plan_fft_forward(config.n_fft);
        Self {
            feature_size: config.feature_size,
            n_fft: config.n_fft,
            hop_length: config.hop_length,
            sample_rate: DEFAULT_SAMPLE_RATE,
            hann_window: hann_window(config.n_fft),
            mel_filters: slaney_mel_filter_bank(config.feature_size, config.n_fft, DEFAULT_SAMPLE_RATE),
            rfft,
        }
    }

    /// 将原始波形转换成模型所需的三维输入特征。
    fn extract(&self, samples: &[f32]) -> Result<Array3<f32>> {
        let spectrogram = self.log_mel_spectrogram(samples)?;
        Ok(spectrogram.insert_axis(Axis(0)))
    }

    /// 计算 log-mel spectrogram。
    fn log_mel_spectrogram(&self, samples: &[f32]) -> Result<Array2<f32>> {
        let padded = reflect_pad(samples, self.n_fft / 2);
        ensure!(padded.len() >= self.n_fft, "wav was too short to build a spectrogram");

        let total_frames = 1 + (padded.len() - self.n_fft) / self.hop_length;
        ensure!(total_frames > 1, "spectrogram needed at least two frames");
        let usable_frames = total_frames - 1;
        let n_freqs = self.n_fft / 2 + 1;

        let mut mel = Array2::<f32>::zeros((usable_frames, self.feature_size));
        let mut input = vec![0_f32; self.n_fft];
        let mut spectrum = self.rfft.make_output_vec();

        for frame_idx in 0..usable_frames {
            let start = frame_idx * self.hop_length;
            let frame = &padded[start..start + self.n_fft];
            for ((slot, sample), window) in input
                .iter_mut()
                .zip(frame.iter().copied())
                .zip(self.hann_window.iter().copied())
            {
                *slot = sample * window;
            }

            self.rfft
                .process(&mut input, &mut spectrum)
                .context("failed to compute FFT for audio frame")?;

            let mut power_spectrum = vec![0_f32; n_freqs];
            for (idx, value) in spectrum.iter().enumerate() {
                power_spectrum[idx] = value.norm_sqr();
            }

            for mel_idx in 0..self.feature_size {
                let filter = self.mel_filters.slice(s![mel_idx, ..]);
                let value = filter
                    .iter()
                    .zip(power_spectrum.iter())
                    .map(|(weight, power)| weight * power)
                    .sum::<f32>();
                mel[[frame_idx, mel_idx]] = value.max(1e-10).log10();
            }
        }

        let max_value = mel.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let floor = max_value - 8.0;
        mel.mapv_inplace(|value| ((value.max(floor)) + 4.0) / 4.0);
        Ok(mel)
    }
}

/// 生成 Hann 窗。
fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|idx| 0.5 - 0.5 * (2.0 * PI * idx as f32 / size as f32).cos())
        .collect()
}

/// 使用反射填充处理边界，尽量贴近 Whisper 的预处理行为。
fn reflect_pad(samples: &[f32], pad: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; pad * 2];
    }

    let mut padded = Vec::with_capacity(samples.len() + 2 * pad);
    for idx in (0..pad).rev() {
        padded.push(samples[reflect_index(-(idx as isize) - 1, samples.len())]);
    }
    padded.extend_from_slice(samples);
    for idx in 0..pad {
        padded.push(samples[reflect_index(samples.len() as isize + idx as isize, samples.len())]);
    }
    padded
}

/// 将越界索引映射到反射后的有效位置。
fn reflect_index(mut idx: isize, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }

    let len = len as isize;
    while idx < 0 || idx >= len {
        if idx < 0 {
            idx = -idx;
        }
        if idx >= len {
            idx = 2 * len - idx - 2;
        }
    }
    idx as usize
}

/// 构造 Slaney 风格的 mel filter bank。
fn slaney_mel_filter_bank(n_mels: usize, n_fft: usize, sample_rate: usize) -> Array2<f32> {
    let n_freqs = n_fft / 2 + 1;
    let mut filters = Array2::<f32>::zeros((n_mels, n_freqs));

    let mel_min = hz_to_mel(0.0);
    let mel_max = hz_to_mel(sample_rate as f32 / 2.0);
    let mel_points = (0..(n_mels + 2))
        .map(|idx| {
            let ratio = idx as f32 / (n_mels + 1) as f32;
            mel_to_hz(mel_min + (mel_max - mel_min) * ratio)
        })
        .collect::<Vec<_>>();
    let fft_freqs = (0..n_freqs)
        .map(|idx| idx as f32 * sample_rate as f32 / n_fft as f32)
        .collect::<Vec<_>>();

    for mel_idx in 0..n_mels {
        let left = mel_points[mel_idx];
        let center = mel_points[mel_idx + 1];
        let right = mel_points[mel_idx + 2];

        for (freq_idx, &freq) in fft_freqs.iter().enumerate() {
            let up_slope = (freq - left) / (center - left);
            let down_slope = (right - freq) / (right - center);
            let value = up_slope.min(down_slope).max(0.0);
            filters[[mel_idx, freq_idx]] = value;
        }

        let enorm = 2.0 / (right - left);
        for freq_idx in 0..n_freqs {
            filters[[mel_idx, freq_idx]] *= enorm;
        }
    }

    filters
}

/// Hz 转 mel。
fn hz_to_mel(hz: f32) -> f32 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4_f32).ln() / 27.0;

    if hz < min_log_hz {
        hz / f_sp
    } else {
        min_log_mel + (hz / min_log_hz).ln() / logstep
    }
}

/// mel 转 Hz。
fn mel_to_hz(mel: f32) -> f32 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4_f32).ln() / 27.0;

    if mel < min_log_mel {
        mel * f_sp
    } else {
        min_log_hz * (logstep * (mel - min_log_mel)).exp()
    }
}

#[derive(Debug, Deserialize)]
struct ModelConfigFile {
    thinker_config: ThinkerConfigFile,
}

#[derive(Debug, Deserialize)]
struct ThinkerConfigFile {
    audio_start_token_id: u32,
    audio_end_token_id: u32,
    audio_token_id: u32,
}

#[derive(Debug, Deserialize)]
struct PreprocessorConfigFile {
    feature_size: usize,
    hop_length: usize,
    n_fft: usize,
}

#[derive(Debug, Deserialize)]
struct TokenizerConfigFile {
    added_tokens_decoder: HashMap<String, AddedTokenFile>,
    eos_token: Option<String>,
    pad_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddedTokenFile {
    content: String,
    lstrip: bool,
    normalized: bool,
    rstrip: bool,
    single_word: bool,
    special: bool,
}
