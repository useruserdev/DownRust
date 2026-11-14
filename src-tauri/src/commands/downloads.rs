// SPDX-License-Identifier: GPL-3.0-or-later
//! Download queue commands.
use downrust_core::model::{
    AddRequest, ChecksumReport, DownloadId, DownloadRecord, HeaderPair, ProbeInfo, QueueMove,
    TargetSuggestion,
};
use tauri::State;

use crate::{error::AppResult, state::AppState};

#[tauri::command]
pub async fn list_downloads(state: State<'_, AppState>) -> AppResult<Vec<DownloadRecord>> {
    Ok(state.manager.list().await?)
}

#[tauri::command]
pub async fn probe_url(
    state: State<'_, AppState>,
    url: String,
    headers: Vec<HeaderPair>,
) -> AppResult<ProbeInfo> {
    Ok(state.manager.probe(&url, &headers).await?)
}

#[tauri::command]
pub async fn suggest_target(
    state: State<'_, AppState>,
    file_name: String,
) -> AppResult<TargetSuggestion> {
    Ok(state.manager.suggest_target(&file_name)?)
}

#[tauri::command]
pub async fn add_download(
    state: State<'_, AppState>,
    request: AddRequest,
) -> AppResult<DownloadRecord> {
    Ok(state.manager.add(request).await?)
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: DownloadId) -> AppResult<()> {
    Ok(state.manager.pause(id).await?)
}

#[tauri::command]
pub async fn resume_download(state: State<'_, AppState>, id: DownloadId) -> AppResult<()> {
    Ok(state.manager.resume(id).await?)
}

#[tauri::command]
pub async fn restart_download(state: State<'_, AppState>, id: DownloadId) -> AppResult<()> {
    Ok(state.manager.restart(id).await?)
}

#[tauri::command]
pub async fn remove_download(
    state: State<'_, AppState>,
    id: DownloadId,
    delete_file: bool,
) -> AppResult<()> {
    Ok(state.manager.remove(id, delete_file).await?)
}

#[tauri::command]
pub async fn pause_all(state: State<'_, AppState>) -> AppResult<()> {
    Ok(state.manager.pause_all().await?)
}

#[tauri::command]
pub async fn resume_all(state: State<'_, AppState>) -> AppResult<()> {
    Ok(state.manager.resume_all().await?)
}

#[tauri::command]
pub async fn set_download_connections(
    state: State<'_, AppState>,
    id: DownloadId,
    connections: u8,
) -> AppResult<()> {
    Ok(state.manager.set_connections(id, connections).await?)
}

#[tauri::command]
pub async fn set_download_speed_limit(
    state: State<'_, AppState>,
    id: DownloadId,
    limit: Option<u64>,
) -> AppResult<()> {
    Ok(state.manager.set_speed_limit(id, limit).await?)
}

#[tauri::command]
pub async fn move_in_queue(
    state: State<'_, AppState>,
    id: DownloadId,
    to: QueueMove,
) -> AppResult<()> {
    Ok(state.manager.move_in_queue(id, to).await?)
}

#[tauri::command]
pub async fn verify_checksum(
    state: State<'_, AppState>,
    id: DownloadId,
    expected: String,
) -> AppResult<ChecksumReport> {
    Ok(state.manager.verify_checksum(id, expected).await?)
}

#[tauri::command]
pub async fn refresh_address(
    state: State<'_, AppState>,
    id: DownloadId,
    url: String,
) -> AppResult<DownloadRecord> {
    Ok(state.manager.refresh_address(id, url).await?)
}
