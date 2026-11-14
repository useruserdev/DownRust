// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod after_queue;
mod app;
mod autostart;
mod bridge;
mod cli;
mod clipboard_watch;
mod commands;
mod dropzone;
mod error;
mod events;
mod i18n;
#[cfg(windows)]
mod jump_list;
mod logging;
mod native_host;
mod notifications;
mod paths;
mod settings;
mod setup;
mod state;
mod taskbar;
#[cfg(windows)]
mod toast;
mod tray;
mod updater;
mod window;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if native_host::is_host_invocation(&args) {
        // The browser talks to this process over stdin and stdout. Errors end the host.
        let _ = native_host::run();
        return;
    }
    app::run();
}
