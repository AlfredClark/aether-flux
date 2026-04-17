// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use serde::Serialize;
use tauri::State;

#[derive(Default)]
struct RecorderState {
    inner: Mutex<Option<ActiveRecording>>,
}

struct ActiveRecording {
    stream: Stream,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    file_path: String,
    sample_rate: u32,
    channels: u16,
}

#[derive(Serialize)]
struct InputDeviceInfo {
    id: String,
    name: String,
    is_default: bool,
}

#[derive(Serialize)]
struct StopRecordingResult {
    file_path: String,
    sample_rate: u32,
    channels: u16,
}

#[tauri::command]
fn list_input_devices() -> Result<Vec<InputDeviceInfo>, String> {
    let host = cpal::default_host();

    let default_name = host.default_input_device().and_then(|d| d.description().ok());

    let devices = host
        .input_devices()
        .map_err(|e| format!("无法获取输入设备列表: {e}"))?;

    let mut result = Vec::new();

    for (index, device) in devices.enumerate() {
        let name = device
            .description().unwrap();

        result.push(InputDeviceInfo {
            id: format!("{index}"),
            is_default: default_name.as_ref().unwrap().name() == Some(&name).unwrap().name(),
            name: name.to_string(),
        });
    }

    Ok(result)
}

fn find_input_device_by_id(device_id: &str) -> Result<Device, String> {
    let host = cpal::default_host();

    let index: usize = device_id
        .parse()
        .map_err(|_| format!("无效的 device_id: {device_id}"))?;

    let mut devices = host
        .input_devices()
        .map_err(|e| format!("无法获取输入设备列表: {e}"))?;

    devices
        .nth(index)
        .ok_or_else(|| format!("未找到对应输入设备: {device_id}"))
}

#[tauri::command]
fn start_recording(
    device_id: String,
    output_path: String,
    recorder: State<'_, RecorderState>,
) -> Result<(), String> {
    let mut guard = recorder
        .inner
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?;

    if guard.is_some() {
        return Err("当前已有录音任务正在进行".into());
    }

    let device = find_input_device_by_id(&device_id)?;
    let supported_config = device
        .default_input_config()
        .map_err(|e| format!("无法获取默认输入配置: {e}"))?;

    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.clone().into();

    let file_path = ensure_wav_path(output_path);
    let writer = create_wav_writer(&file_path, config.channels, config.sample_rate)?;

    let writer = Arc::new(Mutex::new(Some(writer)));
    let writer_for_cb = Arc::clone(&writer);

    let err_fn = |err| eprintln!("音频输入流错误: {err}");

    let stream = match sample_format {
        SampleFormat::I16 => build_stream_i16(&device, &config, writer_for_cb, err_fn)?,
        SampleFormat::U16 => build_stream_u16(&device, &config, writer_for_cb, err_fn)?,
        SampleFormat::F32 => build_stream_f32(&device, &config, writer_for_cb, err_fn)?,
        other => return Err(format!("暂不支持的采样格式: {other:?}")),
    };

    stream.play().map_err(|e| format!("启动录音流失败: {e}"))?;

    *guard = Some(ActiveRecording {
        stream,
        writer,
        file_path,
        sample_rate: config.sample_rate,
        channels: config.channels,
    });

    Ok(())
}

#[tauri::command]
fn stop_recording(recorder: State<'_, RecorderState>) -> Result<StopRecordingResult, String> {
    let mut guard = recorder
        .inner
        .lock()
        .map_err(|_| "录音状态锁定失败".to_string())?;

    let active = guard
        .take()
        .ok_or_else(|| "当前没有正在进行的录音".to_string())?;

    drop(active.stream);

    let mut writer_guard = active
        .writer
        .lock()
        .map_err(|_| "WAV 写入器锁定失败".to_string())?;

    if let Some(writer) = writer_guard.take() {
        writer
            .finalize()
            .map_err(|e| format!("WAV 文件收尾失败: {e}"))?;
    }

    Ok(StopRecordingResult {
        file_path: active.file_path,
        sample_rate: active.sample_rate,
        channels: active.channels,
    })
}

fn ensure_wav_path(output_path: String) -> String {
    if output_path.to_ascii_lowercase().ends_with(".wav") {
        output_path
    } else {
        format!("{output_path}.wav")
    }
}

fn create_wav_writer(
    file_path: &str,
    channels: u16,
    sample_rate: u32,
) -> Result<WavWriter<BufWriter<File>>, String> {
    if let Some(parent) = PathBuf::from(file_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建录音目录失败: {e}"))?;
    }

    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };

    WavWriter::create(file_path, spec).map_err(|e| format!("创建 WAV 文件失败: {e}"))
}

fn build_stream_i16(
    device: &Device,
    config: &StreamConfig,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                if let Ok(mut guard) = writer.lock() {
                    if let Some(writer) = guard.as_mut() {
                        for &sample in data {
                            let _ = writer.write_sample(sample);
                        }
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("构建输入流失败: {e}"))
}

fn build_stream_u16(
    device: &Device,
    config: &StreamConfig,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                if let Ok(mut guard) = writer.lock() {
                    if let Some(writer) = guard.as_mut() {
                        for &sample in data {
                            let s = (sample as i32 - 32768) as i16;
                            let _ = writer.write_sample(s);
                        }
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("构建输入流失败: {e}"))
}

fn build_stream_f32(
    device: &Device,
    config: &StreamConfig,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                if let Ok(mut guard) = writer.lock() {
                    if let Some(writer) = guard.as_mut() {
                        for &sample in data {
                            let s = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            let _ = writer.write_sample(s);
                        }
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("构建输入流失败: {e}"))
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 禁用DMA-BUF渲染，待优化
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_system_fonts::init())
        .plugin(tauri_plugin_audio_recorder::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(RecorderState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            list_input_devices,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
