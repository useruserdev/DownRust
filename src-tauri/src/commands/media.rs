// SPDX-License-Identifier: GPL-3.0-or-later
//! Video (HLS and DASH) helpers: detection of the ffmpeg program.
use downrust_core::media::{detect_ffmpeg as find_ffmpeg, FfmpegInfo};
use tauri::State;

use crate::{error::AppResult, state::AppState};

/// Finds ffmpeg with the current settings. Returns `null` when no program runs.
#[tauri::command]
pub async fn detect_ffmpeg(state: State<'_, AppState>) -> AppResult<Option<FfmpegInfo>> {
    let media = state.manager.settings().media;
    let found = tokio::task::spawn_blocking(move || find_ffmpeg(&media))
        .await
        .ok()
        .flatten();
    Ok(found)
}
