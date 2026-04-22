mod service;

use anyhow::Result;
use tauri::{AppHandle, State};
use tauri_plugin_i18n::PluginI18nExt;

pub use service::{AppSettings, AppSettingsPatch, AppSettingsService, AppSettingsState};

use crate::utils::app_shell::refresh_tray_locale;
use crate::utils::backend_i18n::{localize_error, tr_args};

#[macro_export]
macro_rules! settings_commands {
    ($callback:ident [$($acc:path,)*] $($rest:ident)*) => {
        $callback!(
            [
                $($acc,)*
                $crate::settings::get_app_settings,
                $crate::settings::update_app_settings,
                $crate::settings::reset_app_settings,
            ]
            $($rest)*
        )
    };
}

pub fn initialize_settings_state(app: &AppHandle) -> Result<(AppSettingsState, AppSettings)> {
    let service = AppSettingsService::load_or_init(app)?;
    let settings = service.current();
    Ok((AppSettingsState::with_service(service), settings))
}

pub(crate) fn current_settings(state: &AppSettingsState) -> Result<AppSettings, String> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| "Failed to lock settings state".to_string())?;
    let service = guard
        .as_ref()
        .ok_or_else(|| "Settings service was not initialized".to_string())?;
    Ok(service.current())
}

pub(crate) fn update_settings_with_patch(
    state: &AppSettingsState,
    patch: AppSettingsPatch,
) -> Result<AppSettings, String> {
    let mut guard = state
        .inner
        .lock()
        .map_err(|_| "Failed to lock settings state".to_string())?;
    let service = guard
        .as_mut()
        .ok_or_else(|| "Settings service was not initialized".to_string())?;
    service
        .update(patch)
        .map_err(|err| format!("Failed to update settings: {err:#}"))
}

pub(crate) fn reset_settings(state: &AppSettingsState) -> Result<AppSettings, String> {
    let mut guard = state
        .inner
        .lock()
        .map_err(|_| "Failed to lock settings state".to_string())?;
    let service = guard
        .as_mut()
        .ok_or_else(|| "Settings service was not initialized".to_string())?;
    service
        .reset()
        .map_err(|err| format!("Failed to reset settings: {err:#}"))
}

#[tauri::command]
pub fn get_app_settings(
    settings_state: State<'_, AppSettingsState>,
) -> Result<AppSettings, String> {
    current_settings(&settings_state)
}

#[tauri::command]
pub fn update_app_settings(
    app: AppHandle,
    patch: AppSettingsPatch,
    settings_state: State<'_, AppSettingsState>,
) -> Result<AppSettings, String> {
    let updated = update_settings_with_patch(&settings_state, patch)
        .map_err(|err| localize_error(&app, &err))?;
    app.i18n().set_locale(&updated.locale);
    refresh_tray_locale(&app).map_err(|err| {
        tr_args(
            &app,
            "backend.tray.refresh_failed",
            "Failed to refresh tray locale: {err}",
            &[("err", err.to_string())],
        )
    })?;
    Ok(updated)
}

#[tauri::command]
pub fn reset_app_settings(
    app: AppHandle,
    settings_state: State<'_, AppSettingsState>,
) -> Result<AppSettings, String> {
    let updated = reset_settings(&settings_state).map_err(|err| localize_error(&app, &err))?;
    app.i18n().set_locale(&updated.locale);
    refresh_tray_locale(&app).map_err(|err| {
        tr_args(
            &app,
            "backend.tray.refresh_failed",
            "Failed to refresh tray locale: {err}",
            &[("err", err.to_string())],
        )
    })?;
    Ok(updated)
}
