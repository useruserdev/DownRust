// SPDX-License-Identifier: GPL-3.0-or-later
//! Start-up sequence run from the Tauri setup hook.
use std::sync::{Mutex, RwLock};

use downrust_core::{manager::DownloadManager, store::Store};
use tauri::Manager;
use tokio::sync::mpsc;

use crate::{
    after_queue::AfterQueue, app::MINIMIZED_ARG, autostart, bridge, clipboard_watch,
    error::AppResult, events, logging, paths::AppPaths, settings::AppSettings, state::AppState,
    tray, window,
};

pub fn init(app: &mut tauri::App) -> AppResult<()> {
    let handle = app.handle().clone();
    let paths = AppPaths::resolve(&handle)?;
    logging::init(&paths.log_dir)?;
    let store = Store::open(&paths.db_path)?;
    let mut settings = AppSettings::load_or_default(&store, &paths.downloads_dir)?;
    let tools_dir = paths.data_dir.join("tools");
    std::fs::create_dir_all(&tools_dir)?;
    settings.engine.site.tools_dir = tools_dir.to_string_lossy().into_owned();

    let (tx, rx) = mpsc::unbounded_channel();
    let manager = tauri::async_runtime::block_on(DownloadManager::start(
        store.clone(),
        settings.engine.clone(),
        tx,
    ))?;

    let start_hidden = settings.start_minimized || std::env::args().any(|arg| arg == MINIMIZED_ARG);
    let theme = settings.theme;
    let bridge_enabled = settings.browser_integration;
    let browser_port = settings.browser_port;
    let launch_on_startup = settings.launch_on_startup;
    let show_drop_zone = settings.show_drop_zone;

    app.manage(AppState {
        manager,
        store,
        settings: RwLock::new(settings),
        paths,
        start_hidden,
        bridge: Mutex::new(None),
        after_queue: Mutex::new(AfterQueue::default()),
    });

    if let Err(err) =
        tauri::async_runtime::block_on(bridge::apply(&handle, bridge_enabled, browser_port))
    {
        tracing::warn!("browser bridge did not start: {err}");
    }

    events::spawn_forwarder(handle.clone(), rx);
    tray::create(&handle)?;
    crate::dropzone::apply(&handle, show_drop_zone)?;
    clipboard_watch::spawn(handle.clone());
    autostart::sync(&handle, launch_on_startup)?;
    window::apply_theme(&handle, theme)?;
    Ok(())
}
