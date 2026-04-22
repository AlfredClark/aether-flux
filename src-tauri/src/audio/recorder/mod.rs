mod device;
mod encoding;
mod stream;
mod types;

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat,
};
use tauri::{AppHandle, State};

use crate::app_shell::{hide_recording_status_window, show_recording_status_window};

use self::{
    device::{find_input_device_by_id, list_input_devices_impl, readable_device_name},
    encoding::{create_wav_writer, rewrite_wav_with_sample_rate},
    stream::{build_stream_f32, build_stream_i16, build_stream_u16, SharedWriter},
    types::{ActiveRecording, InputDeviceInfo, RecordingStatus, StopRecordingResult},
};

pub use self::types::RecorderState;

/// 列出当前系统中所有可用的音频输入设备。
#[tauri::command]
pub fn list_input_devices() -> Result<Vec<InputDeviceInfo>, String> {
    list_input_devices_impl()
}

/// 返回当前录音任务的状态快照，供前端查询录音是否正在进行。
#[tauri::command]
pub fn get_recording_status(recorder: State<'_, RecorderState>) -> Result<RecordingStatus, String> {
    let guard = recorder
        .inner
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;

    Ok(RecordingStatus::from_active(guard.active.as_ref()))
}

/// 使用指定输入设备启动录音，并在需要时记录目标输出采样率。
#[tauri::command]
pub fn start_recording(
    device_id: String,
    output_path: String,
    sample_rate: Option<u32>,
    app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<(), String> {
    let mut guard = recorder
        .inner
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;

    if guard.active.is_some() {
        return Err("A recording session is already active".into());
    }

    let device = find_input_device_by_id(&device_id)?;
    let device_name = readable_device_name(&device);
    let supported_config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {e}"))?;

    let sample_format = supported_config.sample_format();
    let config: cpal::StreamConfig = supported_config.into();
    let file_path = encoding::ensure_wav_path(output_path);
    let output_sample_rate = sample_rate.unwrap_or(config.sample_rate);
    let writer = create_wav_writer(&file_path, config.channels, config.sample_rate)?;
    let sink: SharedWriter = Arc::new(Mutex::new(Some(writer)));
    let sink_for_cb = Arc::clone(&sink);

    let err_fn = |err| {
        eprintln!("Audio input stream error: {err}");
    };

    let stream = match sample_format {
        SampleFormat::I16 => build_stream_i16(&device, &config, sink_for_cb, err_fn)?,
        SampleFormat::U16 => build_stream_u16(&device, &config, sink_for_cb, err_fn)?,
        SampleFormat::F32 => build_stream_f32(&device, &config, sink_for_cb, err_fn)?,
        other => return Err(format!("Unsupported sample format: {other:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("Failed to start recording stream: {e}"))?;

    guard.active = Some(ActiveRecording {
        stream,
        sink,
        file_path,
        sample_rate: output_sample_rate,
        input_sample_rate: config.sample_rate,
        channels: config.channels,
        device_id,
        device_name,
        started_at: SystemTime::now(),
    });

    drop(guard);
    show_recording_status_window(&app)
        .map_err(|error| format!("Failed to show recording status window: {error}"))?;

    Ok(())
}

/// 停止当前录音任务，完成文件写入，并在需要时离线重采样输出 WAV。
#[tauri::command]
pub fn stop_recording(
    app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<StopRecordingResult, String> {
    let _ = hide_recording_status_window(&app);
    let mut guard = recorder
        .inner
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;

    let active = guard
        .active
        .take()
        .ok_or_else(|| "There is no active recording".to_string())?;

    let ActiveRecording {
        stream,
        sink,
        file_path,
        sample_rate,
        input_sample_rate,
        channels,
        device_id,
        device_name,
        ..
    } = active;

    let result = StopRecordingResult {
        file_path: file_path.clone(),
        sample_rate,
        channels,
        device_id: device_id.clone(),
        device_name: device_name.clone(),
    };

    drop(stream);

    let mut sink_guard = sink
        .lock()
        .map_err(|_| "Failed to lock WAV writer".to_string())?;

    if let Some(writer) = sink_guard.take() {
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize WAV file: {e}"))?;
    }

    if input_sample_rate != sample_rate {
        rewrite_wav_with_sample_rate(&file_path, channels, input_sample_rate, sample_rate)?;
    }

    drop(sink_guard);

    Ok(result)
}
