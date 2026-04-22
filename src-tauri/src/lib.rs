mod audio;

use audio::asr::wordbank::{
    add_wordbank_entries_from_text, add_wordbank_entry, backup_wordbank_database, clear_wordbank,
    create_wordbank, delete_wordbank, delete_wordbank_entry, delete_wordbank_entry_group,
    export_wordbanks, import_wordbanks, list_enabled_wordbank_homophones, list_wordbank_entries,
    list_wordbanks, reorder_wordbank_entry_group, reorder_wordbanks, reset_wordbank_database,
    set_wordbank_enabled, update_wordbank, update_wordbank_entry, WordbankState,
};
use audio::asr::{
    clear_asr_recording_cache, destroy_asr_model, get_asr_recording_cache_stats, get_asr_status,
    load_asr_model, rebuild_asr_decomposer, rebuild_asr_fitter, recognize_audio, AsrState,
};
use audio::recorder::{
    get_recording_status, list_input_devices, start_recording, stop_recording, RecorderState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 禁用DMA-BUF渲染，待优化
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_system_fonts::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(RecorderState::default())
        .manage(AsrState::default())
        .manage(WordbankState::default())
        .invoke_handler(tauri::generate_handler![
            list_input_devices,
            get_recording_status,
            start_recording,
            stop_recording,
            get_asr_status,
            get_asr_recording_cache_stats,
            clear_asr_recording_cache,
            rebuild_asr_fitter,
            rebuild_asr_decomposer,
            load_asr_model,
            destroy_asr_model,
            recognize_audio,
            list_wordbanks,
            create_wordbank,
            update_wordbank,
            export_wordbanks,
            import_wordbanks,
            backup_wordbank_database,
            reorder_wordbanks,
            set_wordbank_enabled,
            delete_wordbank,
            clear_wordbank,
            list_wordbank_entries,
            list_enabled_wordbank_homophones,
            add_wordbank_entry,
            add_wordbank_entries_from_text,
            update_wordbank_entry,
            delete_wordbank_entry,
            delete_wordbank_entry_group,
            reorder_wordbank_entry_group,
            reset_wordbank_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
