// SPDX-License-Identifier: GPL-3.0-or-later
//! Taskbar button progress: the overall progress of the active downloads.
use std::sync::Mutex;

use downrust_core::model::ProgressBatch;
use tauri::{
    window::{ProgressBarState, ProgressBarStatus},
    AppHandle, Manager,
};

/// What the taskbar button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    None,
    Indeterminate,
    Percent(u8),
}

/// The last value sent to the taskbar. Used to skip repeated calls.
#[derive(Default)]
pub struct TaskbarState(Mutex<Option<Progress>>);

/// Overall progress from `(downloaded, total)` pairs. Items with an unknown or zero total are
/// left out of the percentage. When no total is known the result is indeterminate.
pub fn overall(items: &[(u64, Option<u64>)]) -> Progress {
    if items.is_empty() {
        return Progress::None;
    }
    let mut done: u128 = 0;
    let mut total: u128 = 0;
    for (downloaded, size) in items {
        if let Some(size) = size.filter(|size| *size > 0) {
            done += u128::from((*downloaded).min(size));
            total += u128::from(size);
        }
    }
    if total == 0 {
        return Progress::Indeterminate;
    }
    Progress::Percent((done * 100 / total) as u8)
}

/// Updates the taskbar button from an engine progress batch. The window is called only when the
/// whole percent changes.
pub fn update(app: &AppHandle, batch: &ProgressBatch) {
    let items: Vec<(u64, Option<u64>)> = batch
        .items
        .iter()
        .map(|item| (item.downloaded, item.total))
        .collect();
    let next = overall(&items);
    let state = app.state::<TaskbarState>();
    {
        let mut last = state.0.lock().expect("taskbar lock poisoned");
        if *last == Some(next) {
            return;
        }
        *last = Some(next);
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let (status, progress) = match next {
        Progress::None => (ProgressBarStatus::None, None),
        Progress::Indeterminate => (ProgressBarStatus::Indeterminate, None),
        Progress::Percent(value) => (ProgressBarStatus::Normal, Some(u64::from(value))),
    };
    let result = window.set_progress_bar(ProgressBarState {
        status: Some(status),
        progress,
    });
    if let Err(err) = result {
        tracing::warn!("taskbar progress update failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_items_means_no_progress() {
        assert_eq!(overall(&[]), Progress::None);
    }

    #[test]
    fn unknown_totals_are_indeterminate() {
        assert_eq!(
            overall(&[(10, None), (0, Some(0))]),
            Progress::Indeterminate
        );
    }

    #[test]
    fn percent_sums_the_known_totals() {
        let items = [(50, Some(100)), (100, Some(300)), (7, None)];
        assert_eq!(overall(&items), Progress::Percent(37));
    }

    #[test]
    fn percent_rounds_down_and_caps_at_one_hundred() {
        assert_eq!(overall(&[(2, Some(3))]), Progress::Percent(66));
        assert_eq!(overall(&[(900, Some(100))]), Progress::Percent(100));
    }
}
