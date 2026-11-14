// SPDX-License-Identifier: GPL-3.0-or-later
//! Start-up sequence run from the Tauri setup hook.
use std::{
    path::Path,
    sync::{Mutex, RwLock},
    time::Duration,
};

use downrust_core::{manager::DownloadManager, store::Store};
use tauri::Manager;
use tokio::sync::mpsc;

use crate::{
    after_queue::AfterQueue, app::MINIMIZED_ARG, autostart, bridge, cli, clipboard_watch,
    error::AppResult, events, logging, native_host, paths::AppPaths, settings::AppSettings,
    state::AppState, tray, updater, window,
};

/// Start-up arguments run after the webview has had time to listen for events.
const START_COMMAND_DELAY: Duration = Duration::from_millis(1500);

pub fn init(app: &mut tauri::App) -> AppResult<()> {
    let handle = app.handle().clone();
    let paths = AppPaths::resolve(&handle)?;
    logging::init(&paths.log_dir)?;
    let store = Store::open(&paths.db_path)?;
    let settings = AppSettings::load_or_default(&store, &paths.downloads_dir)?;

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
    let handle_magnet_links = settings.handle_magnet_links;
    let language = settings.language;
    let data_dir = paths.data_dir.clone();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let start_command = cli::parse(&arguments, |path| Path::new(path).is_file());

    app.manage(AppState {
        manager,
        store,
        settings: RwLock::new(settings),
        paths,
        start_hidden,
        bridge: Mutex::new(None),
        after_queue: Mutex::new(AfterQueue::default()),
    });
    app.manage(updater::UpdateState::default());
    app.manage(crate::taskbar::TaskbarState::default());
    cli::register_schemes(&handle, handle_magnet_links);
    #[cfg(windows)]
    crate::jump_list::spawn(crate::i18n::Lang::resolve(language));
    if let Err(err) = native_host::install(&data_dir) {
        tracing::warn!("native messaging manifests were not written: {err}");
    }
    updater::spawn_background(handle.clone());
    if !start_command.is_empty() {
        let delayed = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(START_COMMAND_DELAY).await;
            cli::apply(&delayed, start_command);
        });
    }

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
