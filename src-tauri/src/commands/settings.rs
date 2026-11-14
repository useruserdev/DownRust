// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings read and update commands.
use tauri::{AppHandle, Emitter, State};

use crate::{
    autostart, bridge,
    error::AppResult,
    events::SETTINGS_CHANGED,
    settings::{self, AppSettings},
    state::AppState,
    window,
};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    Ok(state
        .settings
        .read()
        .expect("settings lock poisoned")
        .clone())
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    let mut next = settings.normalized(&state.paths.downloads_dir);
    let previous = state
        .settings
        .read()
        .expect("settings lock poisoned")
        .clone();
    if next.browser_token.is_empty() {
        next.browser_token = previous.browser_token.clone();
    }
    settings::save(&state.store, &next)?;
    state.manager.update_settings(next.engine.clone()).await?;
    *state.settings.write().expect("settings lock poisoned") = next.clone();
    if next.language != previous.language {
        crate::tray::refresh_language(&app);
    }
    if next.browser_integration != previous.browser_integration
        || next.browser_port != previous.browser_port
    {
        bridge::apply(&app, next.browser_integration, next.browser_port).await?;
    }
    autostart::sync(&app, next.launch_on_startup)?;
    window::apply_theme(&app, next.theme)?;
    crate::dropzone::apply(&app, next.show_drop_zone)?;
    app.emit(SETTINGS_CHANGED, next.clone())?;
    Ok(next)
}

/// Replaces the browser token. A running bridge is restarted so it stays on the saved settings.
#[tauri::command]
pub async fn regenerate_browser_token(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    let mut next = state
        .settings
        .read()
        .expect("settings lock poisoned")
        .clone();
    next.browser_token = settings::new_token();
    settings::save(&state.store, &next)?;
    *state.settings.write().expect("settings lock poisoned") = next.clone();
    if bridge::is_running(&app) {
        bridge::apply(&app, true, next.browser_port).await?;
    }
    app.emit(SETTINGS_CHANGED, next.clone())?;
    Ok(next)
}
