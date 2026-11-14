// SPDX-License-Identifier: GPL-3.0-or-later
//! Action that runs when the download queue empties: quit, sleep, hibernate or shut down.
//! The action is kept in memory only and resets after it fires or is cancelled.
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{state::AppState, window};

pub const COUNTDOWN_EVENT: &str = "after-queue-countdown";
pub const CANCELLED_EVENT: &str = "after-queue-cancelled";
pub const COUNTDOWN_SECONDS: u64 = 30;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What happens when the queue empties.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AfterQueueAction {
    #[default]
    None,
    Exit,
    Sleep,
    Hibernate,
    Shutdown,
}

/// The system work that an action asks for once its countdown ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Nothing,
    Quit,
    Suspend { hibernate: bool },
    PowerOff,
}

/// Maps an action to the step that carries it out.
pub fn step_for(action: AfterQueueAction) -> Step {
    match action {
        AfterQueueAction::None => Step::Nothing,
        AfterQueueAction::Exit => Step::Quit,
        AfterQueueAction::Sleep => Step::Suspend { hibernate: false },
        AfterQueueAction::Hibernate => Step::Suspend { hibernate: true },
        AfterQueueAction::Shutdown => Step::PowerOff,
    }
}

/// The chosen action and the countdown that may be running for it.
#[derive(Debug, Default)]
pub struct AfterQueue {
    pub action: AfterQueueAction,
    countdown: Option<u64>,
    next_id: u64,
}

impl AfterQueue {
    /// Starts a countdown for the chosen action. Returns its id, or `None` when no action is set.
    pub fn begin_countdown(&mut self) -> Option<u64> {
        if self.action == AfterQueueAction::None {
            return None;
        }
        self.next_id += 1;
        self.countdown = Some(self.next_id);
        Some(self.next_id)
    }

    /// Ends the countdown with `id` when it is still the current one. Returns the action to
    /// carry out, and clears the action. Stale or cancelled countdowns return `None`.
    pub fn take_if_current(&mut self, id: u64) -> Option<AfterQueueAction> {
        if self.countdown != Some(id) {
            return None;
        }
        self.countdown = None;
        let action = std::mem::take(&mut self.action);
        (action != AfterQueueAction::None).then_some(action)
    }

    /// Clears the action and any running countdown. Returns true when a countdown was running.
    pub fn cancel(&mut self) -> bool {
        self.action = AfterQueueAction::None;
        self.countdown.take().is_some()
    }
}

/// Runs when the queue reports that it is idle. Starts the countdown, then carries out the
/// action when the countdown ends without a cancel.
pub fn on_queue_idle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (id, action) = {
        let mut queue = state.after_queue.lock().expect("after-queue lock poisoned");
        let action = queue.action;
        match queue.begin_countdown() {
            Some(id) => (id, action),
            None => return,
        }
    };
    if let Err(err) = app.emit(
        COUNTDOWN_EVENT,
        serde_json::json!({"action": action, "seconds": COUNTDOWN_SECONDS}),
    ) {
        tracing::warn!("emit {COUNTDOWN_EVENT} failed: {err}");
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(COUNTDOWN_SECONDS)).await;
        let fired = {
            let state = app.state::<AppState>();
            let mut queue = state.after_queue.lock().expect("after-queue lock poisoned");
            queue.take_if_current(id)
        };
        if let Some(action) = fired {
            perform(app, action).await;
        }
    });
}

async fn perform(app: AppHandle, action: AfterQueueAction) {
    match step_for(action) {
        Step::Nothing => {}
        Step::Quit => window::quit_gracefully(app).await,
        Step::Suspend { hibernate } => {
            if let Err(err) = suspend(hibernate) {
                tracing::warn!("cannot sleep the computer: {err}");
            }
        }
        Step::PowerOff => {
            if let Err(err) = power_off() {
                tracing::warn!("cannot shut the computer down: {err}");
            }
        }
    }
}

/// Puts the computer to sleep, or hibernates it. Wake events stay enabled.
#[cfg(windows)]
fn suspend(hibernate: bool) -> std::io::Result<()> {
    use windows_sys::Win32::System::Power::SetSuspendState;
    // SAFETY: the function only takes plain flags and has no pointer arguments.
    let ok = unsafe { SetSuspendState(hibernate, false, false) };
    if !ok {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn suspend(_hibernate: bool) -> std::io::Result<()> {
    Err(std::io::Error::other("sleep is only supported on Windows"))
}

/// Starts the system shutdown with no delay and no console window.
#[cfg(windows)]
fn power_off() -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("shutdown.exe")
        .args(["/s", "/t", "0"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn power_off() -> std::io::Result<()> {
    Err(std::io::Error::other(
        "shutdown is only supported on Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_map_to_steps() {
        assert_eq!(step_for(AfterQueueAction::None), Step::Nothing);
        assert_eq!(step_for(AfterQueueAction::Exit), Step::Quit);
        assert_eq!(
            step_for(AfterQueueAction::Sleep),
            Step::Suspend { hibernate: false }
        );
        assert_eq!(
            step_for(AfterQueueAction::Hibernate),
            Step::Suspend { hibernate: true }
        );
        assert_eq!(step_for(AfterQueueAction::Shutdown), Step::PowerOff);
    }

    #[test]
    fn no_countdown_without_an_action() {
        let mut queue = AfterQueue::default();
        assert_eq!(queue.begin_countdown(), None);
    }

    #[test]
    fn countdown_fires_once_and_resets_the_action() {
        let mut queue = AfterQueue {
            action: AfterQueueAction::Shutdown,
            ..AfterQueue::default()
        };
        let id = queue.begin_countdown().expect("countdown starts");
        assert_eq!(queue.take_if_current(id), Some(AfterQueueAction::Shutdown));
        assert_eq!(queue.action, AfterQueueAction::None);
        assert_eq!(queue.take_if_current(id), None);
    }

    #[test]
    fn cancelled_countdown_never_fires() {
        let mut queue = AfterQueue {
            action: AfterQueueAction::Exit,
            ..AfterQueue::default()
        };
        let id = queue.begin_countdown().expect("countdown starts");
        assert!(queue.cancel());
        assert_eq!(queue.take_if_current(id), None);
        assert!(!queue.cancel());
    }

    #[test]
    fn older_countdown_is_stale() {
        let mut queue = AfterQueue {
            action: AfterQueueAction::Sleep,
            ..AfterQueue::default()
        };
        let first = queue.begin_countdown().expect("first");
        let second = queue.begin_countdown().expect("second");
        assert_eq!(queue.take_if_current(first), None);
        assert_eq!(queue.take_if_current(second), Some(AfterQueueAction::Sleep));
    }
}
