// SPDX-License-Identifier: GPL-3.0-or-later
//! Video site commands: page details and the yt-dlp program.
use std::path::Path;

use downrust_core::http::client::build_client;
use downrust_core::model::HeaderPair;
use downrust_core::site::{
    find_tool, site_info as read_site_info, tool_version, update_tool, SiteInfo,
};
use serde::Serialize;
use tauri::State;

use crate::{
    error::{AppError, AppResult},
    paths::path_string,
    state::AppState,
};

/// The yt-dlp program in use and its version.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteToolStatus {
    pub path: String,
    pub version: String,
}

fn status_of(path: &Path) -> SiteToolStatus {
    SiteToolStatus {
        path: path_string(path),
        version: tool_version(path).unwrap_or_default(),
    }
}

/// Reads the page at the address with yt-dlp.
#[tauri::command]
pub async fn site_info(
    state: State<'_, AppState>,
    url: String,
    headers: Option<Vec<HeaderPair>>,
) -> AppResult<SiteInfo> {
    let settings = state.manager.settings();
    let client = build_client(&settings)?;
    let headers = headers.unwrap_or_default();
    Ok(read_site_info(&settings, &client, &url, &headers).await?)
}

/// The yt-dlp program in use, or `null` when none is found.
#[tauri::command]
pub async fn site_tool_status(state: State<'_, AppState>) -> AppResult<Option<SiteToolStatus>> {
    let settings = state.manager.settings();
    tokio::task::spawn_blocking(move || find_tool(&settings).map(|path| status_of(&path)))
        .await
        .map_err(AppError::message)
}

/// Downloads yt-dlp when missing, updates it, and returns its status.
#[tauri::command]
pub async fn site_tool_update(state: State<'_, AppState>) -> AppResult<SiteToolStatus> {
    let settings = state.manager.settings();
    let client = build_client(&settings)?;
    let path = update_tool(&settings, &client).await?;
    tokio::task::spawn_blocking(move || status_of(&path))
        .await
        .map_err(AppError::message)
}
