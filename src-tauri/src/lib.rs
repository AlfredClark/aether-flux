mod audio;

use audio::asr::{destroy_asr_model, get_asr_status, load_asr_model, recognize_audio, AsrState};
use audio::recorder::{
    get_recording_status, list_input_devices, start_recording, stop_recording, RecorderState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 禁用DMA-BUF渲染，待优化
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_system_fonts::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(RecorderState::default())
        .manage(AsrState::default())
        .invoke_handler(tauri::generate_handler![
            list_input_devices,
            get_recording_status,
            start_recording,
            stop_recording,
            get_asr_status,
            load_asr_model,
            destroy_asr_model,
            recognize_audio
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
