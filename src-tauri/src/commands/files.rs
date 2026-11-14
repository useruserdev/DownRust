// SPDX-License-Identifier: GPL-3.0-or-later
//! Commands that open a completed file or its folder with the operating system.
use downrust_core::model::{DownloadId, DownloadStatus};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    error::{AppError, AppResult},
    paths::path_string,
    state::AppState,
};

#[tauri::command]
pub async fn open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DownloadId,
) -> AppResult<()> {
    let record = state.manager.get(id).await?;
    if record.status != DownloadStatus::Completed {
        return Err(AppError::message("only completed downloads can be opened"));
    }
    let path = path_string(&record.final_path());
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(AppError::message)
}

#[tauri::command]
pub async fn show_in_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DownloadId,
) -> AppResult<()> {
    let record = state.manager.get(id).await?;
    let final_path = record.final_path();
    let opener = app.opener();
    let result = if final_path.exists() {
        opener.reveal_item_in_dir(&final_path)
    } else {
        opener.open_path(record.save_dir.clone(), None::<&str>)
    };
    result.map_err(AppError::message)
}
