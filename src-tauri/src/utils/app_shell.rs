use std::sync::Mutex;

use serde::Serialize;
use tauri::{menu::MenuBuilder, AppHandle, Emitter, Manager, Runtime, State, Window, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::utils::backend_i18n::{tr, tr_args};

const ASR_HOTKEY_EVENT: &str = "asr://hotkey";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const TRAY_MENU_SHOW_ID: &str = "tray-show-main-window";
const TRAY_MENU_QUIT_ID: &str = "tray-quit-app";

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
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

fn translate_or_default<R: Runtime>(app: &AppHandle<R>, key: &str, fallback: &str) -> String {
    tr(app, key, fallback)
}

fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    MenuBuilder::new(app)
        .text(
            TRAY_MENU_SHOW_ID,
            translate_or_default(app, "tray.show_main_window", "Show Main Window"),
        )
        .separator()
        .text(
            TRAY_MENU_QUIT_ID,
            translate_or_default(app, "tray.quit_app", "Quit Application"),
        )
        .build()
}

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;

    let mut tray = tauri::tray::TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(translate_or_default(app, "tray.tooltip", "Aether Flux"))
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

pub fn refresh_tray_locale<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };

    tray.set_menu(Some(build_tray_menu(app)?))?;
    tray.set_tooltip(Some(translate_or_default(
        app,
        "tray.tooltip",
        "Aether Flux",
    )))?;
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
                .map_err(|error| {
                    tr_args(
                        &app,
                        "backend.tray.unregister_hotkey_failed",
                        "Failed to unregister ASR hotkey: {err}",
                        &[("err", error.to_string())],
                    )
                })?;
        }
        return Ok(());
    }

    if registered_shortcut.as_deref() == Some(normalized_shortcut.as_str()) {
        return Ok(());
    }

    if let Some(current_shortcut) = registered_shortcut.take() {
        global_shortcut
            .unregister(current_shortcut.as_str())
            .map_err(|error| {
                tr_args(
                    &app,
                    "backend.tray.unregister_hotkey_failed",
                    "Failed to unregister ASR hotkey: {err}",
                    &[("err", error.to_string())],
                )
            })?;
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
        .map_err(|error| {
            tr_args(
                &app,
                "backend.tray.register_hotkey_failed",
                "Failed to register ASR hotkey: {err}",
                &[("err", error.to_string())],
            )
        })?;

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
