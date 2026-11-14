// SPDX-License-Identifier: GPL-3.0-or-later
//! Statistics command.
use downrust_core::model::StatsSnapshot;
use tauri::State;

use crate::{error::AppResult, state::AppState};

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> AppResult<StatsSnapshot> {
    Ok(state.manager.stats()?)
}
