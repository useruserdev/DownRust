// SPDX-License-Identifier: GPL-3.0-or-later
//! Event names and the forwarder that turns engine events into webview events.
use std::time::{Duration, Instant};

use downrust_core::model::{DownloadKind, EngineEvent, HeaderPair, ProgressBatch};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{i18n, notifications, state::AppState, tray};

pub const DOWNLOAD_ADDED: &str = "download-added";
pub const DOWNLOAD_UPDATED: &str = "download-updated";
pub const DOWNLOAD_COMPLETED: &str = "download-completed";
pub const DOWNLOAD_REMOVED: &str = "download-removed";
pub const DOWNLOAD_PROGRESS: &str = "download-progress";
pub const CLIPBOARD_URL: &str = "clipboard-url";
pub const OPEN_ADD_DIALOG: &str = "open-add-dialog";
pub const SETTINGS_CHANGED: &str = "settings-changed";
pub const EXTERNAL_ADD_REQUEST: &str = "external-add-request";
pub const EXTERNAL_BATCH_REQUEST: &str = "external-batch-request";

#[derive(Clone, Serialize)]
pub struct IdPayload {
    pub id: i64,
}

#[derive(Clone, Serialize)]
pub struct UrlPayload {
    pub url: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAddPayload {
    pub url: String,
    pub file_name: Option<String>,
    pub referrer: Option<String>,
    pub headers: Vec<HeaderPair>,
    pub kind: Option<DownloadKind>,
}

/// One link sent by the browser extension's link grabber.
#[derive(Clone, Serialize, Deserialize)]
pub struct BatchLink {
    pub url: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalBatchPayload {
    pub links: Vec<BatchLink>,
    pub referrer: Option<String>,
    pub headers: Vec<HeaderPair>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAddPayload {
    pub url: Option<String>,
}

/// Spawns the task that reads engine events and emits them to the webview.
pub fn spawn_forwarder(app: AppHandle, mut rx: UnboundedReceiver<EngineEvent>) {
    tauri::async_runtime::spawn(async move {
        let mut last_tooltip: Option<Instant> = None;
        while let Some(event) = rx.recv().await {
            forward(&app, event, &mut last_tooltip);
        }
    });
}

fn forward(app: &AppHandle, event: EngineEvent, last_tooltip: &mut Option<Instant>) {
    match event {
        EngineEvent::Added(record) => emit(app, DOWNLOAD_ADDED, record),
        EngineEvent::Updated(record) => emit(app, DOWNLOAD_UPDATED, record),
        EngineEvent::Removed(id) => emit(app, DOWNLOAD_REMOVED, IdPayload { id }),
        EngineEvent::QueueIdle => crate::after_queue::on_queue_idle(app),
        EngineEvent::Progress(batch) => {
            update_tooltip(app, &batch, last_tooltip);
            emit(app, DOWNLOAD_PROGRESS, batch);
        }
        EngineEvent::Completed(record) => {
            let file_name = record.file_name.clone();
            emit(app, DOWNLOAD_COMPLETED, record);
            let notify = app
                .state::<AppState>()
                .settings
                .read()
                .expect("settings lock poisoned")
                .notify_on_complete;
            if notify {
                notifications::notify_completed(app, &file_name);
            }
        }
    }
}

/// Updates the tray tooltip at most once per second.
fn update_tooltip(app: &AppHandle, batch: &ProgressBatch, last: &mut Option<Instant>) {
    let due = last.is_none_or(|at| at.elapsed() >= Duration::from_secs(1));
    if !due {
        return;
    }
    *last = Some(Instant::now());
    let text = i18n::tray_tooltip(
        i18n::current_lang(app),
        batch.items.len(),
        batch.total_speed,
    );
    tray::set_tooltip(app, &text);
}

fn emit<S: Serialize + Clone>(app: &AppHandle, name: &str, payload: S) {
    if let Err(err) = app.emit(name, payload) {
        tracing::warn!("emit {name} failed: {err}");
    }
}
