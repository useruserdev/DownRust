// SPDX-License-Identifier: GPL-3.0-or-later
//! Desktop notifications. Completions are collected and shown as one summary at most every few
//! seconds: each Windows toast keeps a few system handles, and a burst of finished downloads
//! should not flood the notification center.
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::i18n::{self, Lang, Message};

/// Time to wait for more completions before a summary is shown.
const COLLECT: Duration = Duration::from_secs(2);
/// Shortest time between two completion notifications.
const MIN_GAP: Duration = Duration::from_secs(5);
/// File names listed in a summary before "and N more".
const LISTED: usize = 3;

#[derive(Default)]
struct Pending {
    names: Vec<String>,
    scheduled: bool,
    last_shown: Option<Instant>,
}

static PENDING: Mutex<Pending> = Mutex::new(Pending {
    names: Vec::new(),
    scheduled: false,
    last_shown: None,
});

/// Queues a completion notification for a file. The summary is shown later on the async runtime.
pub fn notify_completed(app: &AppHandle, file_name: &str) {
    let delay = {
        let mut pending = PENDING.lock().expect("notification lock poisoned");
        pending.names.push(file_name.to_owned());
        if pending.scheduled {
            return;
        }
        pending.scheduled = true;
        let since_last = pending.last_shown.map(|at| at.elapsed());
        delay_before_show(since_last)
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        let names = {
            let mut pending = PENDING.lock().expect("notification lock poisoned");
            pending.scheduled = false;
            pending.last_shown = Some(Instant::now());
            std::mem::take(&mut pending.names)
        };
        if names.is_empty() {
            return;
        }
        let (title, body) = summary(i18n::current_lang(&app), &names);
        show(&app, &title, &body);
    });
}

/// Windows: one reused toast notifier (see `toast.rs`). Elsewhere: the notification plugin.
#[cfg(windows)]
fn show(app: &AppHandle, title: &str, body: &str) {
    if let Err(err) = crate::toast::show(&app.config().identifier, title, body) {
        tracing::warn!("completion notification failed: {err}");
    }
}

#[cfg(not(windows))]
fn show(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("completion notification failed: {err}");
    }
}

/// Waits at least `COLLECT`, and longer when the previous notification was less than `MIN_GAP` ago.
fn delay_before_show(since_last: Option<Duration>) -> Duration {
    let gap_left = since_last.map_or(Duration::ZERO, |since| MIN_GAP.saturating_sub(since));
    COLLECT.max(gap_left)
}

/// Title and body for the finished files.
fn summary(lang: Lang, names: &[String]) -> (String, String) {
    if let [only] = names {
        return (
            i18n::text(lang, Message::DownloadComplete).to_owned(),
            only.clone(),
        );
    }
    let title = match lang {
        Lang::En => format!("{} downloads complete", names.len()),
        Lang::Ru => format!("Завершено загрузок: {}", names.len()),
    };
    let mut body = names
        .iter()
        .take(LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let rest = names.len().saturating_sub(LISTED);
    if rest > 0 {
        body.push('\n');
        body.push_str(&match lang {
            Lang::En => format!("and {rest} more"),
            Lang::Ru => format!("и ещё {rest}"),
        });
    }
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn one_file_keeps_the_single_message() {
        let (title, body) = summary(Lang::En, &names(&["a.zip"]));
        assert_eq!(title, "Download complete");
        assert_eq!(body, "a.zip");
    }

    #[test]
    fn several_files_are_summarised() {
        let (title, body) = summary(Lang::Ru, &names(&["a", "b", "c", "d", "e"]));
        assert_eq!(title, "Завершено загрузок: 5");
        assert_eq!(body, "a\nb\nc\nи ещё 2");
        let (title, body) = summary(Lang::En, &names(&["a", "b"]));
        assert_eq!(title, "2 downloads complete");
        assert_eq!(body, "a\nb");
    }

    #[test]
    fn notifications_are_spaced_out() {
        assert_eq!(delay_before_show(None), COLLECT);
        assert_eq!(delay_before_show(Some(Duration::from_secs(60))), COLLECT);
        assert_eq!(
            delay_before_show(Some(Duration::from_secs(1))),
            Duration::from_secs(4)
        );
    }
}
