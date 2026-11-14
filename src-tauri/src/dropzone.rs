// SPDX-License-Identifier: GPL-3.0-or-later
//! Floating drop zone: a small always-on-top window that accepts dropped links.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::{
    error::AppResult,
    events::{OpenAddPayload, OPEN_ADD_DIALOG, SETTINGS_CHANGED},
    settings::{self, AppSettings},
    state::AppState,
    tray, window,
};

/// Window label of the drop zone.
pub const LABEL: &str = "dropzone";
/// Event sent to the main window when several links are dropped. Payload: `{text}`, one link per line.
pub const OPEN_BATCH_DIALOG: &str = "open-batch-dialog";

const SIZE: f64 = 72.0;
const EDGE_MARGIN: f64 = 24.0;
const SAVE_DELAY: Duration = Duration::from_millis(500);
static MOVE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, serde::Serialize)]
pub struct BatchPayload {
    pub text: String,
}

/// Shows the drop zone (creating it when needed) or closes it, and keeps the tray check in step.
pub fn apply(app: &AppHandle, visible: bool) -> AppResult<()> {
    match (app.get_webview_window(LABEL), visible) {
        (Some(drop_window), true) => drop_window.show()?,
        (Some(drop_window), false) => drop_window.close()?,
        (None, true) => create(app)?,
        (None, false) => {}
    }
    tray::set_drop_zone_checked(visible);
    Ok(())
}

/// Saves the flag, then shows or closes the drop zone.
pub fn set_visible(app: &AppHandle, visible: bool) -> AppResult<AppSettings> {
    let next = persist_visible(app, visible)?;
    apply(app, visible)?;
    Ok(next)
}

/// Flips the saved flag. Called from the tray menu on the async runtime, never on the main thread.
pub fn toggle(app: &AppHandle) {
    let visible = app
        .state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .show_drop_zone;
    if let Err(err) = set_visible(app, !visible) {
        tracing::warn!("could not toggle drop zone: {err}");
    }
}

#[tauri::command]
pub async fn set_drop_zone_visible(app: AppHandle, visible: bool) -> AppResult<AppSettings> {
    set_visible(&app, visible)
}

#[tauri::command]
pub async fn show_main(app: AppHandle) {
    window::show_main(&app);
}

/// Opens the Add dialog in the main window for the dropped web links, then shows the main window.
#[tauri::command]
pub async fn handle_dropped_links(app: AppHandle, urls: Vec<String>) -> AppResult<()> {
    let links: Vec<String> = urls.into_iter().filter(|url| is_web_link(url)).collect();
    match links.as_slice() {
        [] => return Ok(()),
        [url] => app.emit_to(
            "main",
            OPEN_ADD_DIALOG,
            OpenAddPayload {
                url: Some(url.clone()),
            },
        )?,
        many => app.emit_to(
            "main",
            OPEN_BATCH_DIALOG,
            BatchPayload {
                text: many.join("\n"),
            },
        )?,
    }
    window::show_main(&app);
    Ok(())
}

fn is_web_link(text: &str) -> bool {
    url::Url::parse(text).is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https"))
}

fn persist_visible(app: &AppHandle, visible: bool) -> AppResult<AppSettings> {
    let state = app.state::<AppState>();
    let next = {
        let mut current = state.settings.write().expect("settings lock poisoned");
        current.show_drop_zone = visible;
        current.clone()
    };
    settings::save(&state.store, &next)?;
    app.emit(SETTINGS_CHANGED, next.clone())?;
    Ok(next)
}

fn create(app: &AppHandle) -> AppResult<()> {
    let drop_window =
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("dropzone.html".into()))
            .title("DownRust drop zone")
            .inner_size(SIZE, SIZE)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .visible(false)
            .disable_drag_drop_handler()
            .build()?;

    let saved = app
        .state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .drop_zone_position;
    let position = match saved {
        Some((x, y)) if is_on_screen(app, x, y) => PhysicalPosition::new(x, y),
        _ => default_position(app)?,
    };
    drop_window.set_position(position)?;
    attach_listeners(app, &drop_window);
    drop_window.show()?;
    Ok(())
}

fn attach_listeners(app: &AppHandle, drop_window: &WebviewWindow) {
    let handle = app.clone();
    drop_window.on_window_event(move |event| match event {
        WindowEvent::Moved(position) => schedule_position_save(&handle, position.x, position.y),
        WindowEvent::CloseRequested { .. } => {
            tray::set_drop_zone_checked(false);
            if let Err(err) = persist_visible(&handle, false) {
                tracing::warn!("could not save drop zone state: {err}");
            }
        }
        _ => {}
    });
}

/// Saves the position once the window has stopped moving for the save delay.
fn schedule_position_save(app: &AppHandle, x: i32, y: i32) {
    let generation = MOVE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAVE_DELAY).await;
        if MOVE_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        save_position(&handle, x, y);
    });
}

fn save_position(app: &AppHandle, x: i32, y: i32) {
    let state = app.state::<AppState>();
    let next = {
        let mut current = state.settings.write().expect("settings lock poisoned");
        current.drop_zone_position = Some((x, y));
        current.clone()
    };
    if let Err(err) = settings::save(&state.store, &next) {
        tracing::warn!("could not save drop zone position: {err}");
    }
}

/// Bottom-right corner of the primary monitor's work area.
fn default_position(app: &AppHandle) -> AppResult<PhysicalPosition<i32>> {
    let Some(monitor) = app.primary_monitor()? else {
        return Ok(PhysicalPosition::new(0, 0));
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let edge = (SIZE * scale).round() as i32;
    let margin = (EDGE_MARGIN * scale).round() as i32;
    let x = area.position.x + area.size.width as i32 - edge - margin;
    let y = area.position.y + area.size.height as i32 - edge - margin;
    Ok(PhysicalPosition::new(x, y))
}

/// True when the point lies inside the work area of one of the connected monitors.
fn is_on_screen(app: &AppHandle, x: i32, y: i32) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let area = monitor.work_area();
        x >= area.position.x
            && y >= area.position.y
            && x < area.position.x + area.size.width as i32
            && y < area.position.y + area.size.height as i32
    })
}
