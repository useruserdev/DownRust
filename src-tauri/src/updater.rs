// SPDX-License-Identifier: GPL-3.0-or-later
//! Application updates: the Tauri updater with a GitHub release fallback, the background check
//! and the install command.
use std::{sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    error::{AppError, AppResult},
    events::{UpdateProgressPayload, UPDATE_AVAILABLE, UPDATE_PROGRESS},
    state::AppState,
};

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(15);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const RELEASE_URL: &str = "https://api.github.com/repos/useruserdev/DownRust/releases/latest";

/// Where the last check found its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateSource {
    Updater,
    Github,
}

/// The result of an update check, sent to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub source: UpdateSource,
}

#[derive(Default)]
struct Slot {
    /// The update found by the last check when the updater found one.
    pending: Option<Update>,
    source: Option<UpdateSource>,
    /// The version the user was last told about in this run.
    notified: Option<String>,
}

/// Update state kept in the Tauri state.
#[derive(Default)]
pub struct UpdateState(Mutex<Slot>);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
}

/// Checks for a new version. The updater goes first. When it fails, the GitHub release is read.
pub async fn check(app: &AppHandle) -> AppResult<UpdateInfo> {
    let current = app.package_info().version.to_string();
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(err) => Err(err),
    };
    let (info, pending) = match found {
        Ok(Some(update)) => (
            UpdateInfo {
                current_version: current,
                available: true,
                version: Some(update.version.clone()),
                notes: update.body.clone().filter(|text| !text.trim().is_empty()),
                date: update.date.map(|date| date.date().to_string()),
                source: UpdateSource::Updater,
            },
            Some(update),
        ),
        Ok(None) => (
            UpdateInfo {
                current_version: current,
                available: false,
                version: None,
                notes: None,
                date: None,
                source: UpdateSource::Updater,
            },
            None,
        ),
        Err(err) => {
            tracing::warn!("updater check failed, reading the GitHub release: {err}");
            (github_info(&current).await?, None)
        }
    };
    let state = app.state::<UpdateState>();
    let mut slot = state.0.lock().expect("update lock poisoned");
    slot.pending = pending;
    slot.source = Some(info.source);
    Ok(info)
}

/// Reads the latest GitHub release and compares its tag with the running version.
async fn github_info(current: &str) -> AppResult<UpdateInfo> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(format!("DownRust/{current}"))
        .build()
        .map_err(AppError::message)?;
    let body = client
        .get(RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(AppError::message)?
        .error_for_status()
        .map_err(AppError::message)?
        .text()
        .await
        .map_err(AppError::message)?;
    let release: Release = serde_json::from_str(&body)?;
    Ok(UpdateInfo {
        current_version: current.to_string(),
        available: is_newer(&release.tag_name, current),
        version: Some(clean_tag(&release.tag_name)),
        notes: release.body.filter(|text| !text.trim().is_empty()),
        date: release
            .published_at
            .map(|date| date.chars().take(10).collect()),
        source: UpdateSource::Github,
    })
}

/// Downloads and installs the update found by the last check, then restarts the app.
pub async fn install(app: &AppHandle) -> AppResult<()> {
    let update = {
        let state = app.state::<UpdateState>();
        let mut slot = state.0.lock().expect("update lock poisoned");
        match slot.pending.take() {
            Some(update) => update,
            None if slot.source == Some(UpdateSource::Github) => {
                return Err(AppError::message(
                    "The update was found on GitHub. Open the download page to install it.",
                ));
            }
            None => {
                return Err(AppError::message(
                    "No update is ready. Check for updates first.",
                ));
            }
        }
    };
    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let payload = UpdateProgressPayload { downloaded, total };
                if let Err(err) = progress_app.emit(UPDATE_PROGRESS, payload) {
                    tracing::warn!("update progress event failed: {err}");
                }
            },
            || {},
        )
        .await
        .map_err(AppError::message)?;
    app.restart()
}

/// Starts the background check: once 15 seconds after start and then every 24 hours. Each
/// round runs only while the `check_updates` setting is on.
pub fn spawn_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            let enabled = app
                .state::<AppState>()
                .settings
                .read()
                .expect("settings lock poisoned")
                .check_updates;
            if enabled {
                announce_if_new(&app).await;
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Runs a check and emits `update-available` once per version in this run.
async fn announce_if_new(app: &AppHandle) {
    let info = match check(app).await {
        Ok(info) => info,
        Err(err) => {
            tracing::warn!("update check failed: {err}");
            return;
        }
    };
    if !info.available {
        return;
    }
    let version = info.version.clone().unwrap_or_default();
    {
        let state = app.state::<UpdateState>();
        let mut slot = state.0.lock().expect("update lock poisoned");
        if slot.notified.as_deref() == Some(version.as_str()) {
            return;
        }
        slot.notified = Some(version);
    }
    if let Err(err) = app.emit(UPDATE_AVAILABLE, info) {
        tracing::warn!("update available event failed: {err}");
    }
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> AppResult<UpdateInfo> {
    check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    install(&app).await
}

/// Opens a link of the DownRust repository in the default browser. Other addresses are refused.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> AppResult<()> {
    let repository = "https://github.com/useruserdev/DownRust";
    if url != repository && !url.starts_with(&format!("{repository}/")) {
        return Err(AppError::message(
            "Only links of the DownRust GitHub repository can be opened here.",
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(AppError::message)
}

/// Removes a leading `v` from a release tag.
fn clean_tag(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

/// True when `candidate` is a newer version than `current`. Versions are compared by their
/// numbers. A release is newer than a pre-release with the same numbers.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(new), Some(old)) => new > old,
        _ => false,
    }
}

/// Reads `v1.2.3` or `1.2.3-beta.1` into the numbers (at least three) and a release flag
/// (1 for a release, 0 for a pre-release).
fn parse_version(text: &str) -> Option<(Vec<u64>, u8)> {
    let trimmed = clean_tag(text);
    let (rest, prerelease) = match trimmed.split_once('-') {
        Some((core, _)) => (core.to_string(), true),
        None => (trimmed.clone(), false),
    };
    let core = rest.split('+').next()?;
    let mut numbers = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    while numbers.len() < 3 {
        numbers.push(0);
    }
    Some((numbers, if prerelease { 0 } else { 1 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_tag_is_detected() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.10.0", "0.9.5"));
        assert!(is_newer("1.0", "0.9.9"));
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
        assert!(!is_newer("v0.9.0", "0.10.0"));
    }

    #[test]
    fn release_beats_pre_release_with_the_same_numbers() {
        assert!(is_newer("v0.2.0", "0.2.0-beta.1"));
        assert!(!is_newer("v0.2.0-beta.2", "0.2.0"));
        assert!(is_newer("v0.2.0-beta.2", "0.1.0"));
    }

    #[test]
    fn unreadable_versions_are_never_newer() {
        assert!(!is_newer("latest", "0.1.0"));
        assert!(!is_newer("v1..2", "0.1.0"));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert!(!is_newer("v0.1.0+build.5", "0.1.0"));
    }
}
