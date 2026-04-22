use std::{fs, path::PathBuf, sync::Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const SETTINGS_FILENAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub locale: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "zh".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub locale: Option<String>,
}

impl AppSettingsPatch {
    pub fn apply_to(self, settings: &mut AppSettings) {
        if let Some(value) = self.locale {
            let value = value.trim();
            if !value.is_empty() {
                settings.locale = value.to_string();
            }
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsFile {
    locale: Option<String>,
}

impl AppSettingsFile {
    fn merge_into(self, settings: &mut AppSettings) {
        AppSettingsPatch {
            locale: self.locale,
        }
        .apply_to(settings);
    }
}

pub struct AppSettingsService {
    path: PathBuf,
    settings: AppSettings,
}

impl AppSettingsService {
    pub fn load_or_init(app: &AppHandle) -> Result<Self> {
        let path = resolve_settings_path(app)?;
        let settings = if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read settings file {}", path.display()))?;
            let file_settings: AppSettingsFile =
                serde_json::from_str(&content).context("failed to decode settings file")?;
            let mut merged = AppSettings::default();
            file_settings.merge_into(&mut merged);
            merged
        } else {
            AppSettings::default()
        };

        let service = Self { path, settings };
        service.save()?;
        Ok(service)
    }

    pub fn current(&self) -> AppSettings {
        self.settings.clone()
    }

    pub fn update(&mut self, patch: AppSettingsPatch) -> Result<AppSettings> {
        patch.apply_to(&mut self.settings);
        self.save()?;
        Ok(self.current())
    }

    pub fn reset(&mut self) -> Result<AppSettings> {
        self.settings = AppSettings::default();
        self.save()?;
        Ok(self.current())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create settings directory {}", parent.display())
            })?;
        }

        let content =
            serde_json::to_string_pretty(&self.settings).context("failed to encode settings")?;
        fs::write(&self.path, content)
            .with_context(|| format!("failed to write settings file {}", self.path.display()))?;
        Ok(())
    }
}

#[derive(Default)]
pub struct AppSettingsState {
    pub(crate) inner: Mutex<Option<AppSettingsService>>,
}

impl AppSettingsState {
    pub fn with_service(service: AppSettingsService) -> Self {
        Self {
            inner: Mutex::new(Some(service)),
        }
    }
}

pub fn resolve_settings_path(app: &AppHandle) -> Result<PathBuf> {
    let base = app
        .path()
        .app_local_data_dir()
        .context("app_local_data_dir was not available")?;
    Ok(base.join(SETTINGS_FILENAME))
}
