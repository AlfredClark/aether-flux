mod audio;
mod settings;
mod utils;

use audio::asr::wordbank::WordbankState;
use audio::asr::AsrState;
use audio::recorder::RecorderState;
use settings::initialize_settings_state;
use tauri::Manager;
use tauri_plugin_i18n::PluginI18nExt;
use utils::app_shell::{
    build_tray, handle_window_event, refresh_tray_locale, AppShellState,
};

macro_rules! collect_invoke_commands {
    ([$($commands:path,)*]) => {
        tauri::generate_handler![$($commands),*]
    };
    ([$($commands:path,)*] $module_macro:ident $($rest:ident)*) => {
        $module_macro!(collect_invoke_commands [$($commands,)*] $($rest)*)
    };
}

macro_rules! app_invoke_handler {
    () => {
        collect_invoke_commands!(
            []
            app_shell_commands
            settings_commands
            recorder_commands asr_commands
        )
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 禁用DMA-BUF渲染，待优化
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    tauri::Builder::default()
        .manage(RecorderState::default())
        .manage(AsrState::default())
        .manage(WordbankState::default())
        .plugin(tauri_plugin_i18n::init(Some("zh".to_string())))
        .setup(|app| {
            let (settings_state, settings) =
                initialize_settings_state(&app.handle()).map_err(|err| err.to_string())?;
            app.manage(settings_state);
            app.manage(AppShellState::default());
            app.i18n().set_locale(&settings.locale);
            build_tray(&app.handle())?;
            refresh_tray_locale(&app.handle())?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_system_fonts::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .on_window_event(|window, event| {
            let state = window.state::<AppShellState>();
            handle_window_event(window, event, state);
        })
        .invoke_handler(app_invoke_handler!())
        .run(tauri::generate_context!("tauri.conf.json"))
        .expect("error while running tauri application");
}
