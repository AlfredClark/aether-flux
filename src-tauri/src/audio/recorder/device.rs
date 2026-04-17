use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device,
};

use super::types::InputDeviceInfo;

/// 枚举当前系统中的输入设备，并标记默认输入设备。
pub fn list_input_devices_impl() -> Result<Vec<InputDeviceInfo>, String> {
    let host = cpal::default_host();
    let default_device_id = host
        .default_input_device()
        .and_then(|device| stable_device_id(&device).ok());
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {e}"))?;

    let mut result = Vec::new();

    for device in devices {
        let id = stable_device_id(&device)?;
        let name = readable_device_name(&device);
        result.push(InputDeviceInfo {
            is_default: default_device_id.as_deref() == Some(id.as_str()),
            id,
            name,
        });
    }

    Ok(result)
}

/// 按稳定设备 ID 查找对应的输入设备句柄。
pub fn find_input_device_by_id(target_id: &str) -> Result<Device, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {e}"))?;

    for device in devices {
        if stable_device_id(&device)? == target_id {
            return Ok(device);
        }
    }

    Err(format!("Input device not found: {target_id}"))
}

/// 为设备生成可用于前后端传递的稳定标识。
fn stable_device_id(device: &Device) -> Result<String, String> {
    device
        .id()
        .map(|id| format!("{id:?}"))
        .map_err(|e| format!("Failed to get device ID: {e}"))
}

/// 返回适合在界面中展示的设备名称。
pub fn readable_device_name(device: &Device) -> String {
    device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "Unknown Input Device".to_string())
}
