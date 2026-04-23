use std::{
    fs::File,
    io::BufWriter,
    sync::{Arc, Mutex},
    time::Instant,
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    BufferSize, Data, SampleFormat, Stream, StreamConfig, SupportedBufferSize,
};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::{
    device::{find_input_device_by_id, list_input_devices_impl, readable_device_name},
    encoding::ensure_wav_path,
};
use crate::utils::backend_i18n::{localize_error, tr, tr_args};

type RecordWriter = WavWriter<BufWriter<File>>;
type SharedRecordCallback = Arc<Mutex<RecordCallbackState>>;

const COMMON_SAMPLE_RATES: &[u32] = &[
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
];
const MIN_RECORD_SAMPLE_RATE: u32 = 8_000;
const MAX_RECORD_SAMPLE_RATE: u32 = 192_000;
const SUPPORTED_RECORD_CHANNELS: &[u16] = &[1, 2];

#[derive(Default)]
pub struct RecordState {
    active: Mutex<Option<ActiveRecord>>,
}

struct ActiveRecord {
    stream: Stream,
    callback: SharedRecordCallback,
    file_path: String,
    device_id: String,
    device_name: String,
    input: RecordInputConfig,
    wav: RecordWavConfig,
    started_at: Instant,
}

struct RecordCallbackState {
    writer: Option<RecordWriter>,
    input_channels: usize,
    output_channels: usize,
    input_sample_rate: u32,
    output_sample_rate: u32,
    wav_sample_format: RecordWavSampleFormat,
    wav_bits_per_sample: u16,
    resample_accumulator: f64,
    written_frames: u64,
    write_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordInputDeviceInfo {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub extended: String,
    pub is_default: bool,
    pub default_config: Option<RecordSupportedInputConfig>,
    pub supported_configs: Vec<RecordSupportedInputConfig>,
    pub config_error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSupportedInputConfig {
    pub channels: u16,
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    pub supported_sample_rates: Vec<u32>,
    pub sample_format: String,
    pub buffer_size: RecordSupportedBufferSize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSupportedBufferSize {
    pub kind: String,
    pub min: Option<u32>,
    pub max: Option<u32>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordInputConfig {
    pub channels: u16,
    pub sample_rate: u32,
    pub sample_format: String,
    pub buffer_size: Option<u32>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordWavConfig {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub sample_format: RecordWavSampleFormat,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordWavSampleFormat {
    Int,
    Float,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordRequest {
    pub device_id: String,
    pub output_path: String,
    pub input: RecordInputConfig,
    pub wav: RecordWavConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRecordResult {
    pub file_path: String,
    pub device_id: String,
    pub device_name: String,
    pub input: RecordInputConfig,
    pub wav: RecordWavConfig,
    pub duration_ms: u128,
    pub written_frames: u64,
}

#[tauri::command]
pub fn list_record_input_devices(app: AppHandle) -> Result<Vec<RecordInputDeviceInfo>, String> {
    let devices = list_input_devices_impl().map_err(|err| localize_error(&app, &err))?;
    let mut result = Vec::with_capacity(devices.len());

    for info in devices {
        let mut default_config = None;
        let mut supported_configs = Vec::new();
        let mut config_error = None;

        match find_input_device_by_id(&info.id) {
            Ok(device) => {
                default_config = device
                    .default_input_config()
                    .ok()
                    .and_then(record_supported_config_from_default);

                match device.supported_input_configs() {
                    Ok(configs) => {
                        supported_configs = configs
                            .filter_map(record_supported_config_from_range)
                            .collect();
                        supported_configs.sort_by_key(|config| {
                            (
                                channel_priority(config.channels),
                                config.sample_format.clone(),
                                config.min_sample_rate,
                            )
                        });
                    }
                    Err(err) => {
                        config_error = Some(tr_args(
                            &app,
                            "backend.recorder.query_input_configs_failed",
                            "Failed to query input configs: {err}",
                            &[("err", err.to_string())],
                        ));
                    }
                }
            }
            Err(err) => {
                config_error = Some(localize_error(&app, &err));
            }
        }

        result.push(RecordInputDeviceInfo {
            id: info.id,
            name: info.name,
            manufacturer: info.manufacturer,
            extended: info.extended,
            is_default: info.is_default,
            default_config,
            supported_configs,
            config_error,
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn start_record(
    request: StartRecordRequest,
    app: AppHandle,
    state: State<'_, RecordState>,
) -> Result<(), String> {
    validate_wav_config(&app, &request.wav)?;

    let mut active_guard = state.active.lock().map_err(|_| {
        tr(
            &app,
            "backend.recorder.lock_failed",
            "Failed to lock recorder state",
        )
    })?;
    if active_guard.is_some() {
        return Err(tr(
            &app,
            "backend.recorder.session_active",
            "A recording session is already active",
        ));
    }

    let device =
        find_input_device_by_id(&request.device_id).map_err(|err| localize_error(&app, &err))?;
    validate_input_config(&app, &device, &request.input)?;

    let file_path = ensure_wav_path(request.output_path.clone());
    let writer =
        create_record_writer(&file_path, &request.wav).map_err(|err| localize_error(&app, &err))?;
    let callback = Arc::new(Mutex::new(RecordCallbackState {
        writer: Some(writer),
        input_channels: request.input.channels as usize,
        output_channels: request.wav.channels as usize,
        input_sample_rate: request.input.sample_rate,
        output_sample_rate: request.wav.sample_rate,
        wav_sample_format: request.wav.sample_format,
        wav_bits_per_sample: request.wav.bits_per_sample,
        resample_accumulator: 0.0,
        written_frames: 0,
        write_error: None,
    }));
    let callback_for_stream = Arc::clone(&callback);
    let sample_format = parse_sample_format(&app, &request.input.sample_format)?;
    let stream_config = StreamConfig {
        channels: request.input.channels,
        sample_rate: request.input.sample_rate,
        buffer_size: match request.input.buffer_size {
            Some(size) => BufferSize::Fixed(size),
            None => BufferSize::Default,
        },
    };
    let err_fn = |err| {
        eprintln!("Configurable audio input stream error: {err}");
    };

    let stream = device
        .build_input_stream_raw(
            &stream_config,
            sample_format,
            move |data, _| write_input_data(data, &callback_for_stream),
            err_fn,
            None,
        )
        .map_err(|e| {
            tr_args(
                &app,
                "backend.recorder.build_stream_failed",
                "Failed to build input stream: {err}",
                &[("err", e.to_string())],
            )
        })?;

    stream.play().map_err(|e| {
        tr_args(
            &app,
            "backend.recorder.start_stream_failed",
            "Failed to start recording stream: {err}",
            &[("err", e.to_string())],
        )
    })?;

    *active_guard = Some(ActiveRecord {
        stream,
        callback,
        file_path,
        device_id: request.device_id,
        device_name: readable_device_name(&device),
        input: request.input,
        wav: request.wav,
        started_at: Instant::now(),
    });

    Ok(())
}

#[tauri::command]
pub fn stop_record(
    app: AppHandle,
    state: State<'_, RecordState>,
) -> Result<StopRecordResult, String> {
    let active = state
        .active
        .lock()
        .map_err(|_| {
            tr(
                &app,
                "backend.recorder.lock_failed",
                "Failed to lock recorder state",
            )
        })?
        .take()
        .ok_or_else(|| {
            tr(
                &app,
                "backend.recorder.no_active_recording",
                "There is no active recording",
            )
        })?;

    let ActiveRecord {
        stream,
        callback,
        file_path,
        device_id,
        device_name,
        input,
        wav,
        started_at,
    } = active;

    drop(stream);

    let mut callback_guard = callback.lock().map_err(|_| {
        tr(
            &app,
            "backend.recorder.wav_writer_lock_failed",
            "Failed to lock WAV writer",
        )
    })?;
    let written_frames = callback_guard.written_frames;
    let write_error = callback_guard.write_error.clone();
    let writer = callback_guard.writer.take().ok_or_else(|| {
        tr(
            &app,
            "backend.recorder.wav_writer_unavailable",
            "WAV writer is not available",
        )
    })?;
    drop(callback_guard);

    writer.finalize().map_err(|e| {
        tr_args(
            &app,
            "backend.recorder.finalize_wav_failed",
            "Failed to finalize WAV file: {err}",
            &[("err", e.to_string())],
        )
    })?;

    if let Some(err) = write_error {
        return Err(localize_error(&app, &err));
    }

    Ok(StopRecordResult {
        file_path,
        device_id,
        device_name,
        input,
        wav,
        duration_ms: started_at.elapsed().as_millis(),
        written_frames,
    })
}

fn record_supported_config_from_default(
    config: cpal::SupportedStreamConfig,
) -> Option<RecordSupportedInputConfig> {
    if config.sample_format().is_dsd() {
        return None;
    }
    let sample_rate = config.sample_rate();
    let channels = config.channels();
    if !is_supported_record_channel(channels) || !is_supported_record_sample_rate(sample_rate) {
        return None;
    }

    Some(RecordSupportedInputConfig {
        channels,
        min_sample_rate: sample_rate,
        max_sample_rate: sample_rate,
        supported_sample_rates: vec![sample_rate],
        sample_format: config.sample_format().to_string(),
        buffer_size: record_buffer_size(config.buffer_size()),
    })
}

fn record_supported_config_from_range(
    config: cpal::SupportedStreamConfigRange,
) -> Option<RecordSupportedInputConfig> {
    let sample_format = config.sample_format();
    if sample_format.is_dsd() {
        return None;
    }
    let channels = config.channels();
    if !is_supported_record_channel(channels) {
        return None;
    }
    let min_sample_rate = config.min_sample_rate().max(MIN_RECORD_SAMPLE_RATE);
    let max_sample_rate = config.max_sample_rate().min(MAX_RECORD_SAMPLE_RATE);
    if min_sample_rate > max_sample_rate {
        return None;
    }
    let supported_sample_rates = candidate_sample_rates(min_sample_rate, max_sample_rate);

    if supported_sample_rates.is_empty() {
        return None;
    }

    Some(RecordSupportedInputConfig {
        channels,
        min_sample_rate,
        max_sample_rate,
        supported_sample_rates,
        sample_format: sample_format.to_string(),
        buffer_size: record_buffer_size(config.buffer_size()),
    })
}

fn candidate_sample_rates(min_sample_rate: u32, max_sample_rate: u32) -> Vec<u32> {
    let mut rates = COMMON_SAMPLE_RATES
        .iter()
        .copied()
        .filter(|rate| *rate >= min_sample_rate && *rate <= max_sample_rate)
        .collect::<Vec<_>>();
    rates.extend([min_sample_rate, max_sample_rate]);
    rates.sort_unstable();
    rates.dedup();
    rates
}

fn is_supported_record_channel(channels: u16) -> bool {
    SUPPORTED_RECORD_CHANNELS.contains(&channels)
}

fn is_supported_record_sample_rate(sample_rate: u32) -> bool {
    (MIN_RECORD_SAMPLE_RATE..=MAX_RECORD_SAMPLE_RATE).contains(&sample_rate)
}

fn channel_priority(channels: u16) -> u8 {
    match channels {
        1 => 0,
        2 => 1,
        _ => 2,
    }
}

fn record_buffer_size(buffer_size: &SupportedBufferSize) -> RecordSupportedBufferSize {
    match *buffer_size {
        SupportedBufferSize::Range { min, max } => RecordSupportedBufferSize {
            kind: "range".to_string(),
            min: Some(min),
            max: Some(max),
        },
        SupportedBufferSize::Unknown => RecordSupportedBufferSize {
            kind: "unknown".to_string(),
            min: None,
            max: None,
        },
    }
}

fn validate_input_config(
    app: &AppHandle,
    device: &cpal::Device,
    input: &RecordInputConfig,
) -> Result<(), String> {
    let sample_format = parse_sample_format(app, &input.sample_format)?;
    if !is_supported_record_channel(input.channels)
        || !is_supported_record_sample_rate(input.sample_rate)
    {
        return Err(tr(
            app,
            "backend.recorder.input_format_not_supported",
            "Selected input format is not supported by this device",
        ));
    }
    let mut matched = false;
    let mut buffer_range = None;

    let configs = device.supported_input_configs().map_err(|e| {
        tr_args(
            app,
            "backend.recorder.query_input_configs_failed",
            "Failed to query input configs: {err}",
            &[("err", e.to_string())],
        )
    })?;
    for config in configs {
        if config.sample_format().is_dsd() {
            continue;
        }
        if config.channels() == input.channels
            && config.sample_format() == sample_format
            && input.sample_rate >= config.min_sample_rate()
            && input.sample_rate <= config.max_sample_rate()
        {
            matched = true;
            buffer_range = Some(*config.buffer_size());
            break;
        }
    }

    if !matched {
        return Err(tr(
            app,
            "backend.recorder.input_format_not_supported",
            "Selected input format is not supported by this device",
        ));
    }

    if let Some(size) = input.buffer_size {
        match buffer_range {
            Some(SupportedBufferSize::Range { min, max }) if size >= min && size <= max => {}
            Some(SupportedBufferSize::Range { min, max }) => {
                return Err(tr_args(
                    app,
                    "backend.recorder.buffer_size_out_of_range",
                    "Selected buffer size is outside the supported range ({min}-{max})",
                    &[("min", min.to_string()), ("max", max.to_string())],
                ));
            }
            _ => {
                return Err(tr(
                    app,
                    "backend.recorder.fixed_buffer_size_unavailable",
                    "This device does not expose a fixed buffer size range; use default",
                ));
            }
        }
    }

    Ok(())
}

fn validate_wav_config(app: &AppHandle, wav: &RecordWavConfig) -> Result<(), String> {
    if wav.channels == 0 {
        return Err(tr(
            app,
            "backend.recorder.wav_channels_invalid",
            "WAV channel count must be greater than zero",
        ));
    }
    if !is_supported_record_sample_rate(wav.sample_rate) {
        return Err(tr(
            app,
            "backend.recorder.wav_sample_rate_invalid",
            "WAV sample rate must be between 8000 and 192000 Hz",
        ));
    }

    match wav.sample_format {
        RecordWavSampleFormat::Int if matches!(wav.bits_per_sample, 8 | 16 | 24 | 32) => Ok(()),
        RecordWavSampleFormat::Float if wav.bits_per_sample == 32 => Ok(()),
        RecordWavSampleFormat::Int => Err(tr(
            app,
            "backend.recorder.wav_int_bits_invalid",
            "Integer WAV output supports 8, 16, 24, or 32 bits",
        )),
        RecordWavSampleFormat::Float => Err(tr(
            app,
            "backend.recorder.wav_float_bits_invalid",
            "Float WAV output supports 32-bit samples only",
        )),
    }
}

fn create_record_writer(file_path: &str, wav: &RecordWavConfig) -> Result<RecordWriter, String> {
    if let Some(parent) = std::path::PathBuf::from(file_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create recording directory: {e}"))?;
    }

    let spec = WavSpec {
        channels: wav.channels,
        sample_rate: wav.sample_rate,
        bits_per_sample: wav.bits_per_sample,
        sample_format: match wav.sample_format {
            RecordWavSampleFormat::Int => WavSampleFormat::Int,
            RecordWavSampleFormat::Float => WavSampleFormat::Float,
        },
    };

    WavWriter::create(file_path, spec).map_err(|e| format!("Failed to create WAV file: {e}"))
}

fn parse_sample_format(app: &AppHandle, value: &str) -> Result<SampleFormat, String> {
    match value {
        "i8" => Ok(SampleFormat::I8),
        "i16" => Ok(SampleFormat::I16),
        "i24" => Ok(SampleFormat::I24),
        "i32" => Ok(SampleFormat::I32),
        "i64" => Ok(SampleFormat::I64),
        "u8" => Ok(SampleFormat::U8),
        "u16" => Ok(SampleFormat::U16),
        "u24" => Ok(SampleFormat::U24),
        "u32" => Ok(SampleFormat::U32),
        "u64" => Ok(SampleFormat::U64),
        "f32" => Ok(SampleFormat::F32),
        "f64" => Ok(SampleFormat::F64),
        other => Err(tr_args(
            app,
            "backend.recorder.unsupported_sample_format",
            "Unsupported sample format: {sample_format}",
            &[("sample_format", other.to_string())],
        )),
    }
}

fn write_input_data(data: &Data, callback: &SharedRecordCallback) {
    let samples = match data.sample_format() {
        SampleFormat::I8 => normalized_samples(data.as_slice::<i8>(), |sample| {
            normalize_signed(sample as i64, 8)
        }),
        SampleFormat::I16 => normalized_samples(data.as_slice::<i16>(), |sample| {
            normalize_signed(sample as i64, 16)
        }),
        SampleFormat::I24 => normalized_samples(data.as_slice::<cpal::I24>(), |sample| {
            normalize_signed(sample.inner() as i64, 24)
        }),
        SampleFormat::I32 => normalized_samples(data.as_slice::<i32>(), |sample| {
            normalize_signed(sample as i64, 32)
        }),
        SampleFormat::I64 => normalized_samples(data.as_slice::<i64>(), |sample| {
            normalize_signed(sample, 64)
        }),
        SampleFormat::U8 => normalized_samples(data.as_slice::<u8>(), |sample| {
            normalize_unsigned(sample as u64, 8)
        }),
        SampleFormat::U16 => normalized_samples(data.as_slice::<u16>(), |sample| {
            normalize_unsigned(sample as u64, 16)
        }),
        SampleFormat::U24 => normalized_samples(data.as_slice::<cpal::U24>(), |sample| {
            normalize_unsigned(sample.inner() as u64, 24)
        }),
        SampleFormat::U32 => normalized_samples(data.as_slice::<u32>(), |sample| {
            normalize_unsigned(sample as u64, 32)
        }),
        SampleFormat::U64 => normalized_samples(data.as_slice::<u64>(), |sample| {
            normalize_unsigned(sample, 64)
        }),
        SampleFormat::F32 => normalized_samples(data.as_slice::<f32>(), |sample| sample as f64),
        SampleFormat::F64 => normalized_samples(data.as_slice::<f64>(), |sample| sample),
        _ => None,
    };

    if let Some(samples) = samples {
        if let Ok(mut state) = callback.lock() {
            state.write_input_samples(&samples);
        }
    }
}

fn normalized_samples<T: Copy>(data: Option<&[T]>, convert: impl Fn(T) -> f64) -> Option<Vec<f64>> {
    data.map(|samples| samples.iter().copied().map(convert).collect())
}

fn normalize_signed(value: i64, bits: u32) -> f64 {
    let scale = ((1_u128 << (bits - 1)) - 1) as f64;
    (value as f64 / scale).clamp(-1.0, 1.0)
}

fn normalize_unsigned(value: u64, bits: u32) -> f64 {
    let center = (1_u128 << (bits - 1)) as f64;
    ((value as f64 - center) / center).clamp(-1.0, 1.0)
}

impl RecordCallbackState {
    fn write_input_samples(&mut self, samples: &[f64]) {
        if self.write_error.is_some() || self.input_channels == 0 {
            return;
        }

        for input_frame in samples.chunks_exact(self.input_channels) {
            let output_frame = convert_channels(input_frame, self.output_channels);
            self.resample_accumulator +=
                self.output_sample_rate as f64 / self.input_sample_rate as f64;

            while self.resample_accumulator >= 1.0 {
                for sample in &output_frame {
                    if let Err(err) = self.write_normalized_sample(*sample) {
                        self.write_error = Some(err);
                        return;
                    }
                }
                self.written_frames += 1;
                self.resample_accumulator -= 1.0;
            }
        }
    }

    fn write_normalized_sample(&mut self, sample: f64) -> Result<(), String> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| "WAV writer is not available".to_string())?;
        let sample = sample.clamp(-1.0, 1.0);

        match (self.wav_sample_format, self.wav_bits_per_sample) {
            (RecordWavSampleFormat::Int, 8) => writer
                .write_sample((sample * i8::MAX as f64).round() as i8)
                .map_err(|e| format!("Failed to write WAV sample: {e}")),
            (RecordWavSampleFormat::Int, 16) => writer
                .write_sample((sample * i16::MAX as f64).round() as i16)
                .map_err(|e| format!("Failed to write WAV sample: {e}")),
            (RecordWavSampleFormat::Int, 24) => writer
                .write_sample((sample * 8_388_607.0).round() as i32)
                .map_err(|e| format!("Failed to write WAV sample: {e}")),
            (RecordWavSampleFormat::Int, 32) => writer
                .write_sample((sample * i32::MAX as f64).round() as i32)
                .map_err(|e| format!("Failed to write WAV sample: {e}")),
            (RecordWavSampleFormat::Float, 32) => writer
                .write_sample(sample as f32)
                .map_err(|e| format!("Failed to write WAV sample: {e}")),
            _ => Err("Unsupported WAV encoding format".to_string()),
        }
    }
}

fn convert_channels(input_frame: &[f64], output_channels: usize) -> Vec<f64> {
    if output_channels == 0 || input_frame.is_empty() {
        return Vec::new();
    }

    if output_channels == 1 {
        let sum = input_frame.iter().sum::<f64>();
        return vec![sum / input_frame.len() as f64];
    }

    let fallback = *input_frame.last().unwrap_or(&0.0);
    (0..output_channels)
        .map(|channel| input_frame.get(channel).copied().unwrap_or(fallback))
        .collect()
}
