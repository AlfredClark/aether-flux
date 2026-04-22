mod device;
mod encoding;
mod stream;
mod types;

use std::sync::{Arc, Mutex};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat,
};
use tauri::{AppHandle, State};

use self::{
    device::{find_input_device_by_id, list_input_devices_impl, readable_device_name},
    encoding::{create_wav_writer, rewrite_wav_with_sample_rate},
    stream::{build_stream_f32, build_stream_i16, build_stream_u16, SharedWriter},
    types::{ActiveRecording, InputDeviceInfo, StopRecordingResult},
};
use crate::utils::backend_i18n::{localize_error, tr, tr_args};

pub use self::types::RecorderState;

#[macro_export]
macro_rules! recorder_commands {
    ($callback:ident [$($acc:path,)*] $($rest:ident)*) => {
        $callback!(
            [
                $($acc,)*
                $crate::audio::recorder::list_input_devices,
                $crate::audio::recorder::start_recording,
                $crate::audio::recorder::stop_recording,
            ]
            $($rest)*
        )
    };
}

/// 列出当前系统中所有可用的音频输入设备。
#[tauri::command]
pub fn list_input_devices(app: AppHandle) -> Result<Vec<InputDeviceInfo>, String> {
    list_input_devices_impl().map_err(|err| localize_error(&app, &err))
}

/// 使用指定输入设备启动录音，并在需要时记录目标输出采样率。
#[tauri::command]
pub fn start_recording(
    device_id: String,
    output_path: String,
    sample_rate: Option<u32>,
    _app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<(), String> {
    let mut guard = recorder.inner.lock().map_err(|_| {
        tr(
            &_app,
            "backend.recorder.lock_failed",
            "Failed to lock recorder state",
        )
    })?;

    if guard.active.is_some() {
        return Err(tr(
            &_app,
            "backend.recorder.session_active",
            "A recording session is already active",
        ));
    }

    let device = find_input_device_by_id(&device_id).map_err(|err| localize_error(&_app, &err))?;
    let device_name = readable_device_name(&device);
    let supported_config = device.default_input_config().map_err(|e| {
        tr_args(
            &_app,
            "backend.recorder.default_input_config_failed",
            "Failed to get default input config: {err}",
            &[("err", e.to_string())],
        )
    })?;

    let sample_format = supported_config.sample_format();
    let config: cpal::StreamConfig = supported_config.into();
    let file_path = encoding::ensure_wav_path(output_path);
    let output_sample_rate = sample_rate.unwrap_or(config.sample_rate);
    let writer = create_wav_writer(&file_path, config.channels, config.sample_rate)
        .map_err(|err| localize_error(&_app, &err))?;
    let sink: SharedWriter = Arc::new(Mutex::new(Some(writer)));
    let sink_for_cb = Arc::clone(&sink);

    let err_fn = |err| {
        eprintln!("Audio input stream error: {err}");
    };

    let stream = match sample_format {
        SampleFormat::I16 => build_stream_i16(&device, &config, sink_for_cb, err_fn)
            .map_err(|err| localize_error(&_app, &err))?,
        SampleFormat::U16 => build_stream_u16(&device, &config, sink_for_cb, err_fn)
            .map_err(|err| localize_error(&_app, &err))?,
        SampleFormat::F32 => build_stream_f32(&device, &config, sink_for_cb, err_fn)
            .map_err(|err| localize_error(&_app, &err))?,
        other => {
            return Err(tr_args(
                &_app,
                "backend.recorder.unsupported_sample_format",
                "Unsupported sample format: {sample_format}",
                &[("sample_format", format!("{other:?}"))],
            ))
        }
    };

    stream.play().map_err(|e| {
        tr_args(
            &_app,
            "backend.recorder.start_stream_failed",
            "Failed to start recording stream: {err}",
            &[("err", e.to_string())],
        )
    })?;

    guard.active = Some(ActiveRecording {
        stream,
        sink,
        file_path,
        sample_rate: output_sample_rate,
        input_sample_rate: config.sample_rate,
        channels: config.channels,
        device_id,
        device_name,
    });

    Ok(())
}

/// 停止当前录音任务，完成文件写入，并在需要时离线重采样输出 WAV。
#[tauri::command]
pub fn stop_recording(
    _app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<StopRecordingResult, String> {
    let mut guard = recorder.inner.lock().map_err(|_| {
        tr(
            &_app,
            "backend.recorder.lock_failed",
            "Failed to lock recorder state",
        )
    })?;

    let active = guard.active.take().ok_or_else(|| {
        tr(
            &_app,
            "backend.recorder.no_active_recording",
            "There is no active recording",
        )
    })?;

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

    let mut sink_guard = sink.lock().map_err(|_| {
        tr(
            &_app,
            "backend.recorder.wav_writer_lock_failed",
            "Failed to lock WAV writer",
        )
    })?;

    if let Some(writer) = sink_guard.take() {
        writer.finalize().map_err(|e| {
            tr_args(
                &_app,
                "backend.recorder.finalize_wav_failed",
                "Failed to finalize WAV file: {err}",
                &[("err", e.to_string())],
            )
        })?;
    }

    if input_sample_rate != sample_rate {
        rewrite_wav_with_sample_rate(&file_path, channels, input_sample_rate, sample_rate)
            .map_err(|err| localize_error(&_app, &err))?;
    }

    drop(sink_guard);

    Ok(result)
}
