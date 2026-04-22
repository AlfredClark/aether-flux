use std::{sync::Mutex, time::SystemTime};

use cpal::Stream;
use serde::Serialize;

use super::stream::SharedWriter;

#[derive(Default)]
pub struct RecorderState {
    pub(crate) inner: Mutex<RecorderInner>,
}

#[derive(Default)]
pub(crate) struct RecorderInner {
    pub(crate) active: Option<ActiveRecording>,
}

pub(crate) struct ActiveRecording {
    pub(crate) stream: Stream,
    pub(crate) sink: SharedWriter,
    pub(crate) file_path: String,
    pub(crate) sample_rate: u32,
    pub(crate) input_sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) started_at: SystemTime,
}

#[derive(Serialize)]
pub struct InputDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize)]
pub struct RecordingStatus {
    pub is_recording: bool,
    pub file_path: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub started_at_millis: Option<u64>,
}

impl RecordingStatus {
    /// 根据当前活动录音对象构造可序列化的状态快照。
    pub(crate) fn from_active(active: Option<&ActiveRecording>) -> Self {
        match active {
            Some(active) => Self {
                is_recording: true,
                file_path: Some(active.file_path.clone()),
                sample_rate: Some(active.sample_rate),
                channels: Some(active.channels),
                device_id: Some(active.device_id.clone()),
                device_name: Some(active.device_name.clone()),
                started_at_millis: active
                    .started_at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_millis() as u64),
            },
            None => Self {
                is_recording: false,
                file_path: None,
                sample_rate: None,
                channels: None,
                device_id: None,
                device_name: None,
                started_at_millis: None,
            },
        }
    }
}

#[derive(Serialize)]
pub struct StopRecordingResult {
    pub file_path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_id: String,
    pub device_name: String,
}
