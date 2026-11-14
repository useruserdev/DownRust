// SPDX-License-Identifier: GPL-3.0-or-later
//! Polls the clipboard and reports copied download links.
use std::time::Duration;

use downrust_core::util::url_detect::looks_like_download_url;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::{
    events::{UrlPayload, CLIPBOARD_URL},
    state::AppState,
};

const POLL_INTERVAL: Duration = Duration::from_millis(800);

/// Starts the polling task.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_seen: Option<String> = None;
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            if !monitor_enabled(&app) {
                last_seen = None;
                continue;
            }
            let Ok(text) = app.clipboard().read_text() else {
                continue;
            };
            let changed = last_seen.as_deref() != Some(text.as_str());
            let seeded = last_seen.is_some();
            if changed {
                let candidate = text.trim();
                if seeded && looks_like_download_url(candidate) {
                    let payload = UrlPayload {
                        url: candidate.to_owned(),
                    };
                    if let Err(err) = app.emit(CLIPBOARD_URL, payload) {
                        tracing::warn!("clipboard event failed: {err}");
                    }
                }
                last_seen = Some(text);
            }
        }
    });
}

fn monitor_enabled(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .clipboard_monitor
}
