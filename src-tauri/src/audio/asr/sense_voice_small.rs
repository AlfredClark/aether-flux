use std::{
    f32::consts::PI,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::loader::{self, AsrLoader, OrtRuntimeConfig, OrtRuntimeConfigProvider};
use anyhow::{anyhow, bail, ensure, Context, Result};
use ndarray::{s, Array1, Array2, Axis, Ix1, Ix3};
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::TensorRef,
};
use realfft::RealFftPlanner;

#[derive(Debug, Clone)]
pub struct SenseVoiceSmallLoaderConfig {
    pub model_path: PathBuf,
    pub tokens_path: PathBuf,
    pub cmvn_path: PathBuf,
    pub sample_rate: usize,
    pub num_mels: usize,
    pub frame_length_ms: usize,
    pub frame_shift_ms: usize,
    pub lfr_m: usize,
    pub lfr_n: usize,
    pub language_token: String,
    pub enable_itn: bool,
    /// 与具体模型无关的 ORT 运行时配置统一收敛到这里。
    pub runtime: OrtRuntimeConfig,
}

impl SenseVoiceSmallLoaderConfig {
    /// 根据 SenseVoiceSmall 模型根目录构造默认配置。
    pub fn from_root(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        Self {
            model_path: root_dir.join("model_quant.onnx"),
            tokens_path: root_dir.join("tokens.json"),
            cmvn_path: root_dir.join("am.mvn"),
            sample_rate: 16_000,
            num_mels: 80,
            frame_length_ms: 25,
            frame_shift_ms: 10,
            lfr_m: 7,
            lfr_n: 6,
            language_token: "<|auto|>".to_string(),
            enable_itn: true,
            runtime: OrtRuntimeConfig {
                optimization_level: GraphOptimizationLevel::Level3,
                ..OrtRuntimeConfig::default()
            },
        }
    }
}

impl OrtRuntimeConfigProvider for SenseVoiceSmallLoaderConfig {
    /// 返回 SenseVoiceSmall 共享的 ORT 运行时配置。
    fn ort_runtime_config(&self) -> &OrtRuntimeConfig {
        &self.runtime
    }
}

#[derive(Debug, Clone)]
pub struct SenseVoiceSmallResult {
    // pub raw_text: String,
    pub text: String,
    // pub special_tokens: Vec<String>,
    // pub token_ids: Vec<usize>,
}

pub struct SenseVoiceSmallLoader {
    session: Session,
    frontend: SenseVoiceFrontend,
    tokens: Vec<String>,
    language_id: i32,
    textnorm_id: i32,
    blank_id: usize,
}

impl SenseVoiceSmallLoader {
    /// 加载 SenseVoice 模型、词表和前端特征提取器。
    pub fn new(config: SenseVoiceSmallLoaderConfig) -> Result<Self> {
        let tokens: Vec<String> = loader::read_json(&config.tokens_path)?;
        let language_id = sensevoice_language_id(&config.language_token)?;
        let textnorm_id = if config.enable_itn { 14 } else { 15 };

        let cmvn = CmvnStats::from_file(&config.cmvn_path)?;
        let frontend = SenseVoiceFrontend::new(
            config.sample_rate,
            config.num_mels,
            config.frame_length_ms,
            config.frame_shift_ms,
            config.lfr_m,
            config.lfr_n,
            cmvn,
        )?;
        let session = loader::build_session(&config, &config.model_path)?;

        Ok(Self {
            session,
            frontend,
            tokens,
            language_id,
            textnorm_id,
            blank_id: 0,
        })
    }

    /// 识别指定 wav 文件，并返回结构化结果。
    pub fn recognize_wav(&mut self, wav_path: impl AsRef<Path>) -> Result<SenseVoiceSmallResult> {
        let wav_path = wav_path.as_ref();
        let samples = loader::load_wav_mono(wav_path, self.frontend.sample_rate)?;
        let speech = self.frontend.extract(&samples)?;
        let speech_len =
            i32::try_from(speech.len_of(Axis(0))).context("speech length overflowed i32")?;
        let speech = speech.insert_axis(Axis(0));
        let speech_lengths = Array1::from_vec(vec![speech_len]);
        let language = Array1::from_vec(vec![self.language_id]);
        let textnorm = Array1::from_vec(vec![self.textnorm_id]);

        let outputs = self.session.run(ort::inputs! {
            "speech" => TensorRef::from_array_view(speech.view())?,
            "speech_lengths" => TensorRef::from_array_view(speech_lengths.view())?,
            "language" => TensorRef::from_array_view(language.view())?,
            "textnorm" => TensorRef::from_array_view(textnorm.view())?
        })?;

        let logits = outputs["ctc_logits"]
            .try_extract_array::<f32>()
            .context("failed to extract ctc_logits")?
            .into_dimensionality::<Ix3>()
            .context("ctc_logits did not have rank 3")?
            .to_owned();
        let encoder_out_lens = outputs["encoder_out_lens"]
            .try_extract_array::<i32>()
            .context("failed to extract encoder_out_lens")?
            .into_dimensionality::<Ix1>()
            .context("encoder_out_lens did not have rank 1")?
            .to_owned();
        drop(outputs);

        let valid_frames =
            usize::try_from(encoder_out_lens[0]).context("encoder_out_lens was negative")?;
        let valid_frames = valid_frames.min(logits.len_of(Axis(1)));

        self.decode_ctc(logits.slice(s![0, 0..valid_frames, ..]))
            .with_context(|| format!("failed to decode {}", wav_path.display()))
    }

    /// 识别指定 wav 文件，并直接返回提取后的文本。
    pub fn recognize_wav_text(&mut self, wav_path: impl AsRef<Path>) -> Result<String> {
        Ok(self.recognize_wav(wav_path)?.text)
    }

    /// 对 CTC logits 执行贪心解码并恢复最终文本。
    fn decode_ctc(&self, logits: ndarray::ArrayView2<'_, f32>) -> Result<SenseVoiceSmallResult> {
        let mut token_ids = Vec::new();
        let mut previous_id = None;

        for frame in logits.axis_iter(Axis(0)) {
            let token_id = loader::argmax(frame);
            if token_id != self.blank_id && Some(token_id) != previous_id {
                token_ids.push(token_id);
            }
            previous_id = Some(token_id);
        }

        let mut text_tokens = Vec::new();
        for &token_id in &token_ids {
            let token = self
                .tokens
                .get(token_id)
                .cloned()
                .ok_or_else(|| anyhow!("token id {token_id} was out of range"))?;

            if token == "<s>" || token == "</s>" || token == "<unk>" {
                continue;
            }
            if !is_special_token(&token) {
                text_tokens.push(token);
            }
        }

        let text = decode_sentencepiece(&text_tokens);
        Ok(SenseVoiceSmallResult { text })
    }
}

impl AsrLoader for SenseVoiceSmallLoader {
    /// 适配统一 loader 接口，转发到 SenseVoiceSmall 的识别实现。
    fn recognize_wav_text(&mut self, wav_path: &Path) -> Result<String> {
        SenseVoiceSmallLoader::recognize_wav_text(self, wav_path)
    }
}

/// 判断 token 是否属于模型输出中的控制类特殊标记。
fn is_special_token(token: &str) -> bool {
    token.starts_with("<|") && token.ends_with("|>")
}

/// 将 sentencepiece token 序列拼接为自然语言文本。
fn decode_sentencepiece(tokens: &[String]) -> String {
    let mut text = String::new();
    for token in tokens {
        if let Some(stripped) = token.strip_prefix('▁') {
            if !text.is_empty() && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push_str(stripped);
        } else {
            text.push_str(token);
        }
    }
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[derive(Debug, Clone)]
struct CmvnStats {
    shift: Vec<f32>,
    scale: Vec<f32>,
}

impl CmvnStats {
    /// 从 Kaldi 风格的 CMVN 文件中读取平移和缩放统计量。
    fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file =
            File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let shift = parse_bracket_values(after_marker(&content, "<AddShift>")?)
            .context("failed to parse <AddShift> vector")?;
        let scale = parse_bracket_values(after_marker(&content, "<Rescale>")?)
            .context("failed to parse <Rescale> vector")?;
        ensure!(
            shift.len() == scale.len(),
            "CMVN shift/scale length mismatch"
        );
        Ok(Self { shift, scale })
    }
}

/// 在原始 CMVN 文本中定位指定标记之后的内容切片。
fn after_marker<'a>(content: &'a str, marker: &str) -> Result<&'a str> {
    let marker_pos = content
        .find(marker)
        .ok_or_else(|| anyhow!("missing marker {marker}"))?;
    Ok(&content[marker_pos + marker.len()..])
}

/// 解析方括号包裹的浮点数序列。
fn parse_bracket_values(content: &str) -> Result<Vec<f32>> {
    let start = content.find('[').ok_or_else(|| anyhow!("missing '['"))?;
    let end = content[start..]
        .find(']')
        .ok_or_else(|| anyhow!("missing ']'"))?
        + start;
    content[start + 1..end]
        .split_whitespace()
        .map(|value| value.parse::<f32>().map_err(|err| anyhow!(err)))
        .collect()
}

/// 将语言标记映射为 SenseVoice 模型要求的语言 ID。
fn sensevoice_language_id(language: &str) -> Result<i32> {
    let normalized = language
        .trim()
        .trim_start_matches("<|")
        .trim_end_matches("|>")
        .to_ascii_lowercase();
    let language_id = match normalized.as_str() {
        "auto" => 0,
        "zh" => 3,
        "en" => 4,
        "yue" => 7,
        "ja" => 11,
        "ko" => 12,
        "nospeech" => 13,
        _ => bail!("unsupported SenseVoice language '{language}'"),
    };
    Ok(language_id)
}

struct SenseVoiceFrontend {
    sample_rate: usize,
    num_mels: usize,
    frame_length: usize,
    frame_shift: usize,
    padded_window_size: usize,
    lfr_m: usize,
    lfr_n: usize,
    window: Vec<f32>,
    mel_filters: Array2<f32>,
    rfft: Arc<dyn realfft::RealToComplex<f32>>,
    cmvn: CmvnStats,
}

impl SenseVoiceFrontend {
    /// 初始化 SenseVoice 前端所需的窗函数、FFT 和 mel 滤波器。
    fn new(
        sample_rate: usize,
        num_mels: usize,
        frame_length_ms: usize,
        frame_shift_ms: usize,
        lfr_m: usize,
        lfr_n: usize,
        cmvn: CmvnStats,
    ) -> Result<Self> {
        let frame_length = sample_rate * frame_length_ms / 1000;
        let frame_shift = sample_rate * frame_shift_ms / 1000;
        let padded_window_size = frame_length.next_power_of_two();
        let mut planner = RealFftPlanner::<f32>::new();
        let rfft = planner.plan_fft_forward(padded_window_size);
        let mel_filters =
            kaldi_mel_filter_bank(num_mels, padded_window_size, sample_rate, 20.0, 0.0);
        ensure!(
            cmvn.shift.len() == num_mels * lfr_m,
            "CMVN shift dim {} did not match expected {}",
            cmvn.shift.len(),
            num_mels * lfr_m
        );
        ensure!(
            cmvn.scale.len() == num_mels * lfr_m,
            "CMVN scale dim {} did not match expected {}",
            cmvn.scale.len(),
            num_mels * lfr_m
        );

        Ok(Self {
            sample_rate,
            num_mels,
            frame_length,
            frame_shift,
            padded_window_size,
            lfr_m,
            lfr_n,
            window: hamming_window(frame_length),
            mel_filters,
            rfft,
            cmvn,
        })
    }

    /// 执行完整的前端特征提取流程。
    fn extract(&self, samples: &[f32]) -> Result<Array2<f32>> {
        let fbank = self.kaldi_fbank(samples)?;
        let lfr = self.apply_lfr(&fbank)?;
        Ok(self.apply_cmvn(lfr))
    }

    /// 计算符合 Kaldi 风格的 log-fbank 特征。
    fn kaldi_fbank(&self, samples: &[f32]) -> Result<Array2<f32>> {
        ensure!(
            samples.len() >= self.frame_length,
            "wav was too short: {} samples, need at least {}",
            samples.len(),
            self.frame_length
        );

        let n_frames = 1 + (samples.len() - self.frame_length) / self.frame_shift;
        let n_freqs = self.padded_window_size / 2 + 1;
        let mut features = Array2::<f32>::zeros((n_frames, self.num_mels));
        let mut fft_input = vec![0.0_f32; self.padded_window_size];
        let mut spectrum = self.rfft.make_output_vec();

        for frame_idx in 0..n_frames {
            let start = frame_idx * self.frame_shift;
            let frame = &samples[start..start + self.frame_length];
            let mean = frame.iter().copied().sum::<f32>() / frame.len() as f32;

            let mut previous = frame[0] - mean;
            for idx in 0..self.frame_length {
                let current = frame[idx] - mean;
                let emphasized = current - 0.97 * previous;
                previous = current;
                fft_input[idx] = emphasized * self.window[idx];
            }
            fft_input[self.frame_length..].fill(0.0);

            self.rfft
                .process(&mut fft_input, &mut spectrum)
                .context("failed to compute FFT for SenseVoice frame")?;

            let mut power_spectrum = vec![0.0_f32; n_freqs];
            for (idx, value) in spectrum.iter().enumerate() {
                power_spectrum[idx] = value.norm_sqr();
            }

            for mel_idx in 0..self.num_mels {
                let filter = self.mel_filters.slice(s![mel_idx, ..]);
                let mel_value = filter
                    .iter()
                    .zip(power_spectrum.iter())
                    .map(|(weight, power)| weight * power)
                    .sum::<f32>()
                    .max(f32::EPSILON)
                    .ln();
                features[[frame_idx, mel_idx]] = mel_value;
            }
        }

        Ok(features)
    }

    /// 应用 Low Frame Rate 叠帧策略以匹配模型输入维度。
    fn apply_lfr(&self, input: &Array2<f32>) -> Result<Array2<f32>> {
        let time_steps = input.len_of(Axis(0));
        let feat_dim = input.len_of(Axis(1));
        ensure!(feat_dim == self.num_mels, "unexpected fbank dim {feat_dim}");
        let output_steps = time_steps.div_ceil(self.lfr_n);
        let left_padding = (self.lfr_m - 1) / 2;

        let mut padded = Array2::<f32>::zeros((time_steps + left_padding, feat_dim));
        for row in 0..left_padding {
            padded
                .slice_mut(s![row, ..])
                .assign(&input.slice(s![0, ..]));
        }
        padded.slice_mut(s![left_padding.., ..]).assign(input);

        let padded_steps = padded.len_of(Axis(0));
        let mut output = Array2::<f32>::zeros((output_steps, feat_dim * self.lfr_m));
        for out_idx in 0..output_steps {
            let start = out_idx * self.lfr_n;
            let remaining = padded_steps.saturating_sub(start);
            let copy_frames = remaining.min(self.lfr_m);
            for offset in 0..copy_frames {
                let src = padded.slice(s![start + offset, ..]);
                output
                    .slice_mut(s![out_idx, offset * feat_dim..(offset + 1) * feat_dim])
                    .assign(&src);
            }
            if copy_frames < self.lfr_m {
                let last = padded.slice(s![padded_steps - 1, ..]).to_owned();
                for offset in copy_frames..self.lfr_m {
                    output
                        .slice_mut(s![out_idx, offset * feat_dim..(offset + 1) * feat_dim])
                        .assign(&last);
                }
            }
        }
        Ok(output)
    }

    /// 对叠帧后的特征执行 CMVN 标准化。
    fn apply_cmvn(&self, mut input: Array2<f32>) -> Array2<f32> {
        for mut row in input.axis_iter_mut(Axis(0)) {
            for (idx, value) in row.iter_mut().enumerate() {
                *value = (*value + self.cmvn.shift[idx]) * self.cmvn.scale[idx];
            }
        }
        input
    }
}

/// 生成 SenseVoice 前端使用的 Hamming 窗。
fn hamming_window(size: usize) -> Vec<f32> {
    let denom = size.saturating_sub(1).max(1) as f32;
    (0..size)
        .map(|idx| 0.54 - 0.46 * (2.0 * PI * idx as f32 / denom).cos())
        .collect()
}

/// 构造 Kaldi 风格的 mel filter bank。
fn kaldi_mel_filter_bank(
    num_mels: usize,
    n_fft: usize,
    sample_rate: usize,
    low_freq: f32,
    high_freq: f32,
) -> Array2<f32> {
    let num_freqs = n_fft / 2 + 1;
    let nyquist = sample_rate as f32 / 2.0;
    let high_freq = if high_freq <= 0.0 {
        nyquist + high_freq
    } else {
        high_freq
    };
    let mel_low = hz_to_kaldi_mel(low_freq);
    let mel_high = hz_to_kaldi_mel(high_freq);
    let mel_step = (mel_high - mel_low) / (num_mels + 1) as f32;

    let mel_points = (0..(num_mels + 2))
        .map(|idx| kaldi_mel_to_hz(mel_low + idx as f32 * mel_step))
        .collect::<Vec<_>>();
    let fft_freqs = (0..num_freqs)
        .map(|idx| idx as f32 * sample_rate as f32 / n_fft as f32)
        .collect::<Vec<_>>();

    let mut filters = Array2::<f32>::zeros((num_mels, num_freqs));
    for mel_idx in 0..num_mels {
        let left = mel_points[mel_idx];
        let center = mel_points[mel_idx + 1];
        let right = mel_points[mel_idx + 2];
        for (freq_idx, freq) in fft_freqs.iter().copied().enumerate() {
            let up = (freq - left) / (center - left);
            let down = (right - freq) / (right - center);
            filters[[mel_idx, freq_idx]] = up.min(down).max(0.0);
        }
    }
    filters
}

/// 将线性频率转换为 Kaldi 定义的 mel 频率。
fn hz_to_kaldi_mel(hz: f32) -> f32 {
    1127.0 * (1.0 + hz / 700.0).ln()
}

/// 将 Kaldi 定义的 mel 频率转换回线性频率。
fn kaldi_mel_to_hz(mel: f32) -> f32 {
    700.0 * ((mel / 1127.0).exp() - 1.0)
}
