use std::sync::Mutex;

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
}

#[derive(Serialize)]
pub struct InputDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize)]
pub struct StopRecordingResult {
    pub file_path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_id: String,
    pub device_name: String,
}
