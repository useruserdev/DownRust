// SPDX-License-Identifier: GPL-3.0-or-later
//! Application information and the frontend start-up signal.
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{error::AppResult, paths::path_string, state::AppState, window};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub log_dir: String,
    pub portable: bool,
}

#[tauri::command]
pub async fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: path_string(&state.paths.data_dir),
        log_dir: path_string(&state.paths.log_dir),
        portable: state.paths.portable,
    })
}

/// Called once by the frontend after it has mounted. Shows the window unless
/// the app was started minimized.
#[tauri::command]
pub async fn frontend_ready(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    if !state.start_hidden {
        window::show_main(&app);
    }
    Ok(())
}
