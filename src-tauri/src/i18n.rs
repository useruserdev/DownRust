// SPDX-License-Identifier: GPL-3.0-or-later
//! Texts the backend shows itself: the tray menu and desktop notifications.
use tauri::{AppHandle, Manager};

use crate::{settings::LanguagePreference, state::AppState};

/// The language of the backend texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    /// Turns the saved preference into a language. `System` reads the Windows UI language.
    pub fn resolve(preference: LanguagePreference) -> Self {
        match preference {
            LanguagePreference::En => Self::En,
            LanguagePreference::Ru => Self::Ru,
            LanguagePreference::System => match sys_locale::get_locale() {
                Some(locale) if locale.to_ascii_lowercase().starts_with("ru") => Self::Ru,
                _ => Self::En,
            },
        }
    }
}

/// The language that applies right now, from the stored settings.
pub fn current_lang(app: &AppHandle) -> Lang {
    let preference = app
        .state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .language;
    Lang::resolve(preference)
}

/// The keys of the texts the backend shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Show,
    AddDownload,
    PauseAll,
    ResumeAll,
    DropZone,
    Quit,
    DownloadComplete,
    NewDownload,
}

/// The tray tooltip: how many downloads run and their total speed.
pub fn tray_tooltip(lang: Lang, downloading: usize, bytes_per_sec: u64) -> String {
    match lang {
        Lang::En => format!(
            "DownRust: {downloading} downloading, {}",
            speed(lang, bytes_per_sec)
        ),
        Lang::Ru => format!(
            "DownRust: загружается {downloading}, {}",
            speed(lang, bytes_per_sec)
        ),
    }
}

/// A speed in base-1024 units, with the decimal comma in Russian.
fn speed(lang: Lang, bytes_per_sec: u64) -> String {
    let units: [&str; 4] = match lang {
        Lang::En => ["B", "KB", "MB", "GB"],
        Lang::Ru => ["Б", "КБ", "МБ", "ГБ"],
    };
    let per_second = match lang {
        Lang::En => "/s",
        Lang::Ru => "/с",
    };
    let mut value = bytes_per_sec as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let number = if unit == 0 {
        bytes_per_sec.to_string()
    } else {
        format!("{value:.1}")
    };
    let number = match lang {
        Lang::En => number,
        Lang::Ru => number.replace('.', ","),
    };
    format!("{number} {}{per_second}", units[unit])
}

/// Returns the text of a key in a language.
pub fn text(lang: Lang, key: Message) -> &'static str {
    match (lang, key) {
        (Lang::En, Message::Show) => "Show DownRust",
        (Lang::En, Message::AddDownload) => "Add download...",
        (Lang::En, Message::PauseAll) => "Pause all",
        (Lang::En, Message::ResumeAll) => "Resume all",
        (Lang::En, Message::DropZone) => "Drop zone",
        (Lang::En, Message::Quit) => "Quit",
        (Lang::En, Message::DownloadComplete) => "Download complete",
        (Lang::En, Message::NewDownload) => "New download",
        (Lang::Ru, Message::Show) => "Показать DownRust",
        (Lang::Ru, Message::AddDownload) => "Добавить загрузку...",
        (Lang::Ru, Message::PauseAll) => "Пауза для всех",
        (Lang::Ru, Message::ResumeAll) => "Продолжить все",
        (Lang::Ru, Message::DropZone) => "Зона перетаскивания",
        (Lang::Ru, Message::Quit) => "Выход",
        (Lang::Ru, Message::DownloadComplete) => "Загрузка завершена",
        (Lang::Ru, Message::NewDownload) => "Новая загрузка",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_uses_the_language_units() {
        assert_eq!(
            tray_tooltip(Lang::En, 2, 1_572_864),
            "DownRust: 2 downloading, 1.5 MB/s"
        );
        assert_eq!(
            tray_tooltip(Lang::Ru, 2, 1_572_864),
            "DownRust: загружается 2, 1,5 МБ/с"
        );
        assert_eq!(
            tray_tooltip(Lang::Ru, 0, 0),
            "DownRust: загружается 0, 0 Б/с"
        );
    }
}
