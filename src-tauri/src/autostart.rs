// SPDX-License-Identifier: GPL-3.0-or-later
//! Launch-at-login registration.
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::error::{AppError, AppResult};

/// Enables or disables launch at login so it matches `enabled`.
pub fn sync(app: &AppHandle, enabled: bool) -> AppResult<()> {
    let launcher = app.autolaunch();
    let current = launcher.is_enabled().map_err(AppError::message)?;
    if current == enabled {
        return Ok(());
    }
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(AppError::message)
}
