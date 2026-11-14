// SPDX-License-Identifier: GPL-3.0-or-later
//! Notification-area icon with its context menu.
use std::sync::OnceLock;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

use crate::{
    dropzone,
    error::AppResult,
    events::{OpenAddPayload, OPEN_ADD_DIALOG},
    i18n::{self, Message},
    state::AppState,
    window,
};

pub const TRAY_ID: &str = "main-tray";
const DROP_ZONE_ID: &str = "drop_zone";

/// The menu items, kept so their labels can change with the language.
struct Labels {
    show: MenuItem<Wry>,
    add: MenuItem<Wry>,
    pause_all: MenuItem<Wry>,
    resume_all: MenuItem<Wry>,
    drop_zone: CheckMenuItem<Wry>,
    quit: MenuItem<Wry>,
}

static LABELS: OnceLock<Labels> = OnceLock::new();

pub fn create(app: &AppHandle) -> AppResult<()> {
    let lang = i18n::current_lang(app);
    let labels = Labels {
        show: MenuItem::with_id(
            app,
            "show",
            i18n::text(lang, Message::Show),
            true,
            None::<&str>,
        )?,
        add: MenuItem::with_id(
            app,
            "add",
            i18n::text(lang, Message::AddDownload),
            true,
            None::<&str>,
        )?,
        pause_all: MenuItem::with_id(
            app,
            "pause_all",
            i18n::text(lang, Message::PauseAll),
            true,
            None::<&str>,
        )?,
        resume_all: MenuItem::with_id(
            app,
            "resume_all",
            i18n::text(lang, Message::ResumeAll),
            true,
            None::<&str>,
        )?,
        quit: MenuItem::with_id(
            app,
            "quit",
            i18n::text(lang, Message::Quit),
            true,
            None::<&str>,
        )?,
        drop_zone: CheckMenuItem::with_id(
            app,
            DROP_ZONE_ID,
            i18n::text(lang, Message::DropZone),
            true,
            drop_zone_on(app),
            None::<&str>,
        )?,
    };
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &labels.show,
            &labels.add,
            &sep1,
            &labels.pause_all,
            &labels.resume_all,
            &sep2,
            &labels.drop_zone,
            &sep3,
            &labels.quit,
        ],
    )?;
    // The items are kept so the check mark can follow the drop zone when it is shown or hidden elsewhere.
    let _ = LABELS.set(labels);

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("DownRust")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| on_tray_icon_event(tray.app_handle(), event));
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// Sets every menu label to the current language. Called after the language setting changes.
pub fn refresh_language(app: &AppHandle) {
    let Some(labels) = LABELS.get() else {
        return;
    };
    let lang = i18n::current_lang(app);
    let results = [
        labels.show.set_text(i18n::text(lang, Message::Show)),
        labels.add.set_text(i18n::text(lang, Message::AddDownload)),
        labels
            .pause_all
            .set_text(i18n::text(lang, Message::PauseAll)),
        labels
            .resume_all
            .set_text(i18n::text(lang, Message::ResumeAll)),
        labels
            .drop_zone
            .set_text(i18n::text(lang, Message::DropZone)),
        labels.quit.set_text(i18n::text(lang, Message::Quit)),
    ];
    for result in results {
        if let Err(err) = result {
            tracing::warn!("could not update tray label: {err}");
        }
    }
}

/// Sets the tray tooltip text.
pub fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(err) = tray.set_tooltip(Some(text)) {
            tracing::warn!("could not set tray tooltip: {err}");
        }
    }
}

/// Sets the check mark of the drop zone tray item.
pub fn set_drop_zone_checked(checked: bool) {
    let Some(labels) = LABELS.get() else {
        return;
    };
    if let Err(err) = labels.drop_zone.set_checked(checked) {
        tracing::warn!("could not update drop zone tray item: {err}");
    }
}

fn drop_zone_on(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .show_drop_zone
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => window::show_main(app),
        "add" => {
            window::show_main(app);
            let payload = OpenAddPayload { url: None };
            if let Err(err) = app.emit(OPEN_ADD_DIALOG, payload) {
                tracing::warn!("could not open add dialog: {err}");
            }
        }
        "pause_all" => pause_or_resume_all(app, true),
        "resume_all" => pause_or_resume_all(app, false),
        DROP_ZONE_ID => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move { dropzone::toggle(&handle) });
        }
        "quit" => {
            tauri::async_runtime::spawn(window::quit_gracefully(app.clone()));
        }
        _ => {}
    }
}

fn pause_or_resume_all(app: &AppHandle, pause: bool) {
    let manager = app.state::<AppState>().manager.clone();
    tauri::async_runtime::spawn(async move {
        let result = if pause {
            manager.pause_all().await
        } else {
            manager.resume_all().await
        };
        if let Err(err) = result {
            tracing::warn!("tray pause/resume failed: {err}");
        }
    });
}

fn on_tray_icon_event(app: &AppHandle, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        window::show_main(app);
    }
}
