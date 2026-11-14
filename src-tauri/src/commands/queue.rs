// SPDX-License-Identifier: GPL-3.0-or-later
//! Commands for the action that runs when the download queue empties.
use tauri::{AppHandle, Emitter, State};

use crate::{
    after_queue::{AfterQueueAction, CANCELLED_EVENT},
    error::AppResult,
    state::AppState,
};

#[tauri::command]
pub async fn set_after_queue_action(
    state: State<'_, AppState>,
    action: AfterQueueAction,
) -> AppResult<()> {
    state
        .after_queue
        .lock()
        .expect("after-queue lock poisoned")
        .action = action;
    Ok(())
}

#[tauri::command]
pub async fn get_after_queue_action(state: State<'_, AppState>) -> AppResult<AfterQueueAction> {
    Ok(state
        .after_queue
        .lock()
        .expect("after-queue lock poisoned")
        .action)
}

/// Clears the action. A running countdown stops and reports `after-queue-cancelled`.
#[tauri::command]
pub async fn cancel_after_queue(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let was_running = state
        .after_queue
        .lock()
        .expect("after-queue lock poisoned")
        .cancel();
    if was_running {
        app.emit(CANCELLED_EVENT, ())?;
    }
    Ok(())
}
