use anyhow::{bail, ensure, Context};
use std::path::Path;

/// 加载wav文件并转换为mono
pub(crate) fn load_wav_mono(path: &Path, expected_sample_rate: usize) -> anyhow::Result<Vec<f32>> {
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
            .collect::<anyhow::Result<Vec<_>, _>>()
            .context("failed to read f32 wav samples")?,
        (hound::SampleFormat::Int, bits_per_sample) if bits_per_sample <= 16 => {
            let scale = f32::from(i16::MAX);
            reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| f32::from(value) / scale))
                .collect::<anyhow::Result<Vec<_>, _>>()
                .context("failed to read i16 wav samples")?
        }
        (hound::SampleFormat::Int, bits_per_sample) if bits_per_sample <= 32 => {
            let scale = ((1_i64 << (bits_per_sample - 1)) - 1) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<anyhow::Result<Vec<_>, _>>()
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
