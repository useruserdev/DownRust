// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod after_queue;
mod app;
mod autostart;
mod bridge;
mod clipboard_watch;
mod commands;
mod dropzone;
mod error;
mod events;
mod i18n;
mod logging;
mod notifications;
mod paths;
mod settings;
mod setup;
mod state;
#[cfg(windows)]
mod toast;
mod tray;
mod window;

fn main() {
    app::run();
}
