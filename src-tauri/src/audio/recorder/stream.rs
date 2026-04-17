use std::{
    fs::File,
    io::BufWriter,
    sync::{Arc, Mutex},
};

use cpal::{traits::DeviceTrait, Device, Stream, StreamConfig};
use hound::WavWriter;

pub type SharedWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

/// 为 `i16` 输入样本构建录音流，并将数据直接写入共享 WAV 写入器。
pub fn build_stream_i16(
    device: &Device,
    config: &StreamConfig,
    sink: SharedWriter,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| write_i16(&sink, data),
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {e}"))
}

/// 为 `u16` 输入样本构建录音流，并在写入前转换为 `i16` PCM。
pub fn build_stream_u16(
    device: &Device,
    config: &StreamConfig,
    sink: SharedWriter,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| write_u16(&sink, data),
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {e}"))
}

/// 为 `f32` 输入样本构建录音流，并在写入前转换为 `i16` PCM。
pub fn build_stream_f32(
    device: &Device,
    config: &StreamConfig,
    sink: SharedWriter,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| write_f32(&sink, data),
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {e}"))
}

/// 在持有写入器锁的前提下执行一次写入操作。
fn with_writer<F>(sink: &SharedWriter, mut f: F)
where
    F: FnMut(&mut WavWriter<BufWriter<File>>),
{
    if let Ok(mut guard) = sink.lock() {
        if let Some(writer) = guard.as_mut() {
            f(writer);
        }
    }
}

/// 将 `i16` 样本批量写入 WAV 文件。
fn write_i16(sink: &SharedWriter, data: &[i16]) {
    with_writer(sink, |writer| {
        for &sample in data {
            let _ = writer.write_sample(sample);
        }
    });
}

/// 将 `u16` 样本映射到 `i16` 范围后写入 WAV 文件。
fn write_u16(sink: &SharedWriter, data: &[u16]) {
    with_writer(sink, |writer| {
        for &sample in data {
            let normalized = (sample as i32 - 32768) as i16;
            let _ = writer.write_sample(normalized);
        }
    });
}

/// 将 `f32` 样本裁剪并缩放到 `i16` 范围后写入 WAV 文件。
fn write_f32(sink: &SharedWriter, data: &[f32]) {
    with_writer(sink, |writer| {
        for &sample in data {
            let normalized = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            let _ = writer.write_sample(normalized);
        }
    });
}
