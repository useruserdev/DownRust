// SPDX-License-Identifier: GPL-3.0-or-later
//! Main window visibility, close behaviour, theme and graceful quit.
use std::time::Duration;

use tauri::{AppHandle, Manager, Window, WindowEvent};

use crate::{error::AppResult, settings::Theme, state::AppState};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Shows, restores and focuses the main window.
pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let result = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(err) = result {
        tracing::warn!("could not show main window: {err}");
    }
}

/// Hides to the tray when close-to-tray is on, otherwise quits.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    api.prevent_close();
    let app = window.app_handle().clone();
    let close_to_tray = app
        .state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .close_to_tray;
    if close_to_tray {
        if let Err(err) = window.hide() {
            tracing::warn!("could not hide main window: {err}");
        }
    } else {
        tauri::async_runtime::spawn(quit_gracefully(app));
    }
}

/// Stops the download manager (at most five seconds), then exits.
pub async fn quit_gracefully(app: AppHandle) {
    let manager = app.state::<AppState>().manager.clone();
    match tokio::time::timeout(SHUTDOWN_TIMEOUT, manager.shutdown()).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::warn!("manager shutdown failed: {err}"),
        Err(_) => tracing::warn!("manager shutdown timed out"),
    }
    app.exit(0);
}

/// Applies the theme to the title bar. `System` follows the operating system.
pub fn apply_theme(app: &AppHandle, theme: Theme) -> AppResult<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let value = match theme {
        Theme::System => None,
        Theme::Dark => Some(tauri::Theme::Dark),
        Theme::Light => Some(tauri::Theme::Light),
    };
    window.set_theme(value)?;
    Ok(())
}
