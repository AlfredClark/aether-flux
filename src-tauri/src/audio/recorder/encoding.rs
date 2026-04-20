use std::path::PathBuf;

use hound::{SampleFormat as WavSampleFormat, WavReader, WavSpec, WavWriter};

/// 确保输出路径带有 `.wav` 后缀。
pub fn ensure_wav_path(output_path: String) -> String {
    if output_path.to_ascii_lowercase().ends_with(".wav") {
        output_path
    } else {
        format!("{output_path}.wav")
    }
}

/// 创建用于写入 PCM WAV 数据的文件写入器。
pub fn create_wav_writer(
    file_path: &str,
    channels: u16,
    sample_rate: u32,
) -> Result<WavWriter<std::io::BufWriter<std::fs::File>>, String> {
    if let Some(parent) = PathBuf::from(file_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create recording directory: {e}"))?;
    }

    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };

    WavWriter::create(file_path, spec).map_err(|e| format!("Failed to create WAV file: {e}"))
}

/// 以离线方式将现有 WAV 文件重采样到目标采样率并覆盖原文件。
pub fn rewrite_wav_with_sample_rate(
    file_path: &str,
    channels: u16,
    input_sample_rate: u32,
    output_sample_rate: u32,
) -> Result<(), String> {
    let mut reader =
        WavReader::open(file_path).map_err(|e| format!("Failed to open source WAV file: {e}"))?;
    let input_samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read source WAV samples: {e}"))?;
    let output_samples = resample_interleaved_i16(
        &input_samples,
        channels as usize,
        input_sample_rate,
        output_sample_rate,
    );
    let tmp_path = format!("{file_path}.resample.tmp");
    let mut writer = create_wav_writer(&tmp_path, channels, output_sample_rate)?;

    for sample in output_samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write resampled WAV data: {e}"))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize resampled WAV file: {e}"))?;

    if PathBuf::from(file_path).exists() {
        std::fs::remove_file(file_path)
            .map_err(|e| format!("Failed to remove original WAV file: {e}"))?;
    }

    std::fs::rename(&tmp_path, file_path)
        .map_err(|e| format!("Failed to replace WAV file with resampled output: {e}"))?;
    Ok(())
}

/// 对交错排列的 PCM `i16` 采样做线性插值重采样。
fn resample_interleaved_i16(
    input: &[i16],
    channels: usize,
    input_sample_rate: u32,
    output_sample_rate: u32,
) -> Vec<i16> {
    if channels == 0 || input.is_empty() || input_sample_rate == output_sample_rate {
        return input.to_vec();
    }

    let input_frames = input.len() / channels;
    if input_frames <= 1 {
        return input.to_vec();
    }

    let output_frames = ((input_frames as u64 * output_sample_rate as u64)
        / input_sample_rate as u64)
        .max(1) as usize;
    let step = input_sample_rate as f64 / output_sample_rate as f64;
    let mut output = Vec::with_capacity(output_frames * channels);

    for output_frame in 0..output_frames {
        let source_pos = output_frame as f64 * step;
        let left_frame = source_pos.floor() as usize;
        let right_frame = (left_frame + 1).min(input_frames - 1);
        let frac = (source_pos - left_frame as f64) as f32;

        for channel in 0..channels {
            let left = input[left_frame * channels + channel] as f32;
            let right = input[right_frame * channels + channel] as f32;
            let sample = left + (right - left) * frac;
            output.push(sample.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }

    output
}
