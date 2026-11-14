// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri builder: plugins, command registration and window hooks.
use tauri_plugin_window_state::StateFlags;

pub const MINIMIZED_ARG: &str = "--minimized";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::window::show_main(app)
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_ARG]),
        ))
        .setup(|app| {
            crate::setup::init(app)?;
            Ok(())
        })
        .on_window_event(crate::window::on_window_event)
        .invoke_handler(tauri::generate_handler![
            crate::commands::downloads::list_downloads,
            crate::commands::downloads::probe_url,
            crate::commands::downloads::suggest_target,
            crate::commands::downloads::add_download,
            crate::commands::media::detect_ffmpeg,
            crate::commands::downloads::pause_download,
            crate::commands::downloads::resume_download,
            crate::commands::downloads::restart_download,
            crate::commands::downloads::remove_download,
            crate::commands::downloads::pause_all,
            crate::commands::downloads::resume_all,
            crate::commands::downloads::set_download_connections,
            crate::commands::downloads::set_download_speed_limit,
            crate::commands::downloads::move_in_queue,
            crate::commands::downloads::verify_checksum,
            crate::commands::downloads::refresh_address,
            crate::commands::files::open_file,
            crate::commands::files::show_in_folder,
            crate::commands::settings::get_settings,
            crate::commands::settings::update_settings,
            crate::commands::settings::regenerate_browser_token,
            crate::commands::queue::set_after_queue_action,
            crate::commands::queue::get_after_queue_action,
            crate::commands::queue::cancel_after_queue,
            crate::commands::credentials::set_proxy_password,
            crate::commands::credentials::set_site_password,
            crate::commands::stats::get_stats,
            crate::commands::app::get_app_info,
            crate::commands::app::frontend_ready,
            crate::dropzone::set_drop_zone_visible,
            crate::dropzone::show_main,
            crate::dropzone::handle_dropped_links,
        ])
        .run(tauri::generate_context!())
        .expect("DownRust failed to start");
}
