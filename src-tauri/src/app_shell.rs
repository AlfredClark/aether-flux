use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    menu::MenuBuilder, AppHandle, Emitter, Manager, Runtime, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_positioner::{Position, WindowExt};

const ASR_HOTKEY_EVENT: &str = "asr://hotkey";
const MAIN_WINDOW_LABEL: &str = "main";
const RECORDING_STATUS_WINDOW_LABEL: &str = "recording-status";
const TRAY_ID: &str = "main-tray";
const TRAY_MENU_SHOW_ID: &str = "tray-show-main-window";
const TRAY_MENU_QUIT_ID: &str = "tray-quit-app";
const RECORDING_STATUS_WIDTH: f64 = 160.0;
const RECORDING_STATUS_HEIGHT: f64 = 40.0;

#[derive(Default)]
pub struct AppShellState {
    tray_mode_enabled: Mutex<bool>,
    registered_asr_hotkey: Mutex<Option<String>>,
    quit_requested: Mutex<bool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AsrHotkeyPayload {
    shortcut: String,
    state: String,
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    hide_recording_status_window(app)?;
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

fn position_recording_status_window<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    if window.current_monitor()?.is_some() {
        window.move_window(Position::BottomCenter)?;
    }

    Ok(())
}

fn get_recording_status_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    if let Some(window) = app.get_webview_window(RECORDING_STATUS_WINDOW_LABEL) {
        return Ok(window);
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        RECORDING_STATUS_WINDOW_LABEL,
        WebviewUrl::App("recording-status".into()),
    )
    .title("Recording Status")
    .visible(false)
    .focused(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .decorations(false)
    .transparent(true)
    .inner_size(RECORDING_STATUS_WIDTH, RECORDING_STATUS_HEIGHT);

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon)?;
    }

    builder.build()
}

pub fn build_recording_status_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let _ = get_recording_status_window(app)?;
    Ok(())
}

pub fn show_recording_status_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let window = get_recording_status_window(app)?;
    window.show()?;
    position_recording_status_window(&window)?;
    Ok(())
}

pub fn hide_recording_status_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(RECORDING_STATUS_WINDOW_LABEL) {
        window.hide()?;
    }
    Ok(())
}

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_MENU_SHOW_ID, "显示主窗口")
        .separator()
        .text(TRAY_MENU_QUIT_ID, "退出应用")
        .build()?;

    let mut tray = tauri::tray::TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Aether Flux")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW_ID => {
                let _ = show_main_window(app);
            }
            TRAY_MENU_QUIT_ID => {
                if let Some(state) = app.try_state::<AppShellState>() {
                    let mut quit_requested = state.quit_requested.lock().unwrap();
                    *quit_requested = true;
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == tauri::tray::MouseButton::Left
                    && button_state == tauri::tray::MouseButtonState::Up
                {
                    let _ = show_main_window(&tray.app_handle());
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

pub fn handle_window_event<R: Runtime>(
    window: &Window<R>,
    event: &WindowEvent,
    state: State<'_, AppShellState>,
) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        let tray_mode_enabled = *state.tray_mode_enabled.lock().unwrap();
        let quit_requested = *state.quit_requested.lock().unwrap();
        if tray_mode_enabled && !quit_requested {
            api.prevent_close();
            let _ = hide_recording_status_window(&window.app_handle());
            let _ = window.hide();
        }
    }
}

#[tauri::command]
pub fn configure_asr_hotkey<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppShellState>,
    enabled: bool,
    shortcut: String,
) -> Result<(), String> {
    let normalized_shortcut = shortcut.trim().to_string();
    let global_shortcut = app.global_shortcut();
    let mut registered_shortcut = state.registered_asr_hotkey.lock().unwrap();

    if !enabled || normalized_shortcut.is_empty() {
        if let Some(current_shortcut) = registered_shortcut.take() {
            global_shortcut
                .unregister(current_shortcut.as_str())
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }

    if registered_shortcut.as_deref() == Some(normalized_shortcut.as_str()) {
        return Ok(());
    }

    if let Some(current_shortcut) = registered_shortcut.take() {
        global_shortcut
            .unregister(current_shortcut.as_str())
            .map_err(|error| error.to_string())?;
    }

    global_shortcut
        .on_shortcut(normalized_shortcut.as_str(), move |app, shortcut, event| {
            let payload = AsrHotkeyPayload {
                shortcut: shortcut.to_string(),
                state: match event.state {
                    ShortcutState::Pressed => "pressed".to_string(),
                    ShortcutState::Released => "released".to_string(),
                },
            };
            let _ = app.emit(ASR_HOTKEY_EVENT, payload);
        })
        .map_err(|error| error.to_string())?;

    *registered_shortcut = Some(normalized_shortcut);
    Ok(())
}

#[tauri::command]
pub fn set_tray_mode_enabled(state: State<'_, AppShellState>, enabled: bool) {
    let mut tray_mode_enabled = state.tray_mode_enabled.lock().unwrap();
    *tray_mode_enabled = enabled;
}

#[tauri::command]
pub fn get_tray_mode_enabled(state: State<'_, AppShellState>) -> bool {
    *state.tray_mode_enabled.lock().unwrap()
}
