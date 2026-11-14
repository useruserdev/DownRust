// SPDX-License-Identifier: GPL-3.0-or-later
//! Commands that store proxy and site passwords in the system credential store.
use downrust_core::secrets;
use tauri::State;

use crate::{error::AppResult, state::AppState};

/// Stores the proxy password. `null` deletes it. The download client is rebuilt so the new
/// password is used by the next request.
#[tauri::command]
pub async fn set_proxy_password(
    state: State<'_, AppState>,
    password: Option<String>,
) -> AppResult<()> {
    secrets::set(secrets::PROXY_USER, password.as_deref())?;
    let engine = state
        .settings
        .read()
        .expect("settings lock poisoned")
        .engine
        .clone();
    state.manager.update_settings(engine).await?;
    Ok(())
}

/// Stores the password of the site login for `host`. `null` deletes it.
#[tauri::command]
pub async fn set_site_password(host: String, password: Option<String>) -> AppResult<()> {
    secrets::set(&secrets::site_user(&host), password.as_deref())?;
    Ok(())
}
