// SPDX-License-Identifier: GPL-3.0-or-later
//! Application settings stored as JSON next to the download database.
use std::path::Path;

use downrust_core::{settings::EngineSettings, store::Store};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;

pub const DEFAULT_BROWSER_PORT: u16 = 52151;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Dark,
    Light,
}

/// The language choice. `System` follows the Windows UI language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    #[default]
    System,
    En,
    Ru,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub engine: EngineSettings,
    pub theme: Theme,
    pub language: LanguagePreference,
    pub clipboard_monitor: bool,
    pub start_minimized: bool,
    pub launch_on_startup: bool,
    pub close_to_tray: bool,
    pub notify_on_complete: bool,
    pub browser_integration: bool,
    pub browser_port: u16,
    pub browser_token: String,
    pub show_drop_zone: bool,
    pub drop_zone_position: Option<(i32, i32)>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            engine: EngineSettings::default(),
            theme: Theme::System,
            language: LanguagePreference::System,
            clipboard_monitor: false,
            start_minimized: false,
            launch_on_startup: false,
            close_to_tray: true,
            notify_on_complete: true,
            browser_integration: false,
            browser_port: DEFAULT_BROWSER_PORT,
            browser_token: String::new(),
            show_drop_zone: false,
            drop_zone_position: None,
        }
    }
}

impl AppSettings {
    /// Fills empty folders from the downloads root and clamps the engine values.
    pub fn normalized(mut self, downloads_dir: &Path) -> Self {
        self.engine.fill_missing_dirs(downloads_dir);
        self.engine = self.engine.sanitized();
        self
    }

    /// Reads the saved settings, or defaults when none exist or the JSON is unreadable.
    /// Creates and saves a browser token when none exists.
    pub fn load_or_default(store: &Store, downloads_dir: &Path) -> AppResult<Self> {
        let mut settings = store
            .load_settings_json()?
            .and_then(|json| serde_json::from_str::<Self>(&json).ok())
            .unwrap_or_default()
            .normalized(downloads_dir);
        if settings.browser_token.is_empty() {
            settings.browser_token = new_token();
            save(store, &settings)?;
        }
        Ok(settings)
    }
}

/// Writes the settings to the database.
pub fn save(store: &Store, settings: &AppSettings) -> AppResult<()> {
    let json = serde_json::to_string(settings)?;
    store.save_settings_json(&json)?;
    Ok(())
}

pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
