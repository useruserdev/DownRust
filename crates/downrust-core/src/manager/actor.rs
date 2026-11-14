// SPDX-License-Identifier: GPL-3.0-or-later
//! Manager actor: one task owns the scheduling state and handles every command in arrival order.
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, NaiveDateTime};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::MissedTickBehavior;

use super::active::ActiveTask;
use super::command::{Cmd, ResolvedAdd};
use super::naming::resolve_target;
use super::Shared;
use crate::engine::file::remove_if_exists;
use crate::engine::limiter::RateLimiter;
use crate::error::{Error, Result};
use crate::http::client::build_client;
use crate::model::{DownloadId, DownloadRecord, DownloadStatus, EngineEvent, NewDownload};
use crate::settings::{EngineSettings, MAX_CONNECTIONS};
use crate::util::time::now_ms;

/// State owned by the actor task. Handlers never await; they reply through their oneshot channel.
pub(super) struct ManagerActor {
    pub(super) shared: Arc<Shared>,
    pub(super) events: UnboundedSender<EngineEvent>,
    pub(super) self_tx: UnboundedSender<Cmd>,
    pub(super) global_limiter: Arc<RateLimiter>,
    /// The rate last given to `global_limiter` by the speed schedule check.
    pub(super) applied_rate: Cell<Option<u64>>,
    pub(super) active: HashMap<DownloadId, ActiveTask>,
    /// Downloads waiting for their task to end before removal; the value says whether to delete the file.
    pub(super) removing: HashMap<DownloadId, bool>,
    pub(super) restarting: HashSet<DownloadId>,
    pub(super) shutdown_reply: Option<oneshot::Sender<()>>,
    pub(super) shutting_down: bool,
    pub(super) sent_empty_progress: bool,
    /// Transfers cancelled because the time window closed. They return to the queue.
    pub(super) window_stop: HashSet<DownloadId>,
    /// True once the queue has had something active or queued since the last `QueueIdle`.
    pub(super) queue_busy: bool,
    /// True when a download completed since the last `QueueIdle`.
    pub(super) completed_in_run: bool,
}

/// How often the time window is checked while the manager runs.
const WINDOW_CHECK: Duration = Duration::from_secs(30);

impl ManagerActor {
    pub(super) fn new(
        shared: Arc<Shared>,
        events: UnboundedSender<EngineEvent>,
        self_tx: UnboundedSender<Cmd>,
        global_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            shared,
            events,
            self_tx,
            global_limiter,
            applied_rate: Cell::new(None),
            active: HashMap::new(),
            removing: HashMap::new(),
            restarting: HashSet::new(),
            shutdown_reply: None,
            shutting_down: false,
            sent_empty_progress: false,
            window_stop: HashSet::new(),
            queue_busy: false,
            completed_in_run: false,
        }
    }

    /// Runs until shutdown finishes or every sender is gone.
    pub(super) async fn run(mut self, mut rx: UnboundedReceiver<Cmd>) {
        let period = Duration::from_millis(self.shared.settings().progress_interval_ms);
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut window = tokio::time::interval(WINDOW_CHECK);
        window.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(cmd) => {
                        let stop = self.handle(cmd);
                        self.check_queue_idle();
                        if stop {
                            break;
                        }
                    }
                    None => break,
                },
                _ = tick.tick() => self.emit_progress(),
                _ = window.tick() => {
                    self.schedule();
                    self.apply_speed_limit();
                }
            }
        }
    }

    pub(super) fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    pub(super) fn load(&self, id: DownloadId) -> Result<DownloadRecord> {
        self.shared
            .store
            .get_download(id)?
            .ok_or(Error::NotFound(id))
    }

    /// Stores a new status and error text, then sends the updated record.
    pub(super) fn set_status(
        &self,
        id: DownloadId,
        status: DownloadStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.shared.store.update_status(id, status, error)?;
        self.emit_updated(id)
    }

    pub(super) fn emit_updated(&self, id: DownloadId) -> Result<()> {
        let record = self.load(id)?;
        self.emit(EngineEvent::Updated(record));
        Ok(())
    }

    pub(super) fn cancel_task(&self, id: DownloadId) {
        if let Some(task) = self.active.get(&id) {
            task.cancel.cancel();
        }
    }

    /// Overwrites the stored byte count and size with the live values of a running transfer.
    fn merge_live(&self, record: &mut DownloadRecord) {
        if let Some(task) = self.active.get(&record.id) {
            record.downloaded = task.live.downloaded();
            if let Some(total) = task.live.total() {
                record.total_size = Some(total);
            }
        }
    }

    pub(super) fn add(&mut self, add: ResolvedAdd) -> Result<DownloadRecord> {
        let settings = self.shared.settings();
        let probe = add.probe.as_ref();
        let target = resolve_target(
            &self.shared.store,
            &settings,
            &add.file_name,
            add.req.save_dir.as_deref(),
        )?;
        let new = NewDownload {
            url: add.req.url.trim().to_owned(),
            final_url: probe.map(|p| p.final_url.clone()),
            referrer: add.req.referrer.clone(),
            file_name: target.file_name,
            save_dir: target.save_dir,
            category: target.category,
            status: if add.req.start {
                DownloadStatus::Queued
            } else {
                DownloadStatus::Paused
            },
            total_size: probe.and_then(|p| p.total_size),
            resumable: probe.is_some_and(|p| p.resumable),
            connections: add
                .req
                .connections
                .unwrap_or(settings.default_connections)
                .clamp(1, MAX_CONNECTIONS),
            speed_limit: add.req.speed_limit,
            etag: probe.and_then(|p| p.etag.clone()),
            last_modified: probe.and_then(|p| p.last_modified.clone()),
            content_type: probe.and_then(|p| p.content_type.clone()),
            headers: add.req.headers.clone(),
            queue_position: self.shared.store.next_queue_position()?,
            created_at: now_ms(),
            expected_sha256: add.req.expected_sha256.clone(),
            kind: add.req.kind.unwrap_or_default(),
            mirrors: add.req.mirrors.clone(),
            options: add.req.options.clone(),
        };
        let record = self.shared.store.insert_download(&new)?;
        self.emit(EngineEvent::Added(record.clone()));
        self.schedule();
        Ok(record)
    }

    pub(super) fn list(&self) -> Result<Vec<DownloadRecord>> {
        let mut records = self.shared.store.list_downloads()?;
        for record in &mut records {
            self.merge_live(record);
        }
        Ok(records)
    }

    pub(super) fn get(&self, id: DownloadId) -> Result<DownloadRecord> {
        let mut record = self.load(id)?;
        self.merge_live(&mut record);
        Ok(record)
    }

    pub(super) fn restart(&mut self, id: DownloadId) -> Result<()> {
        self.load(id)?;
        if self.active.contains_key(&id) {
            self.restarting.insert(id);
            self.cancel_task(id);
            return Ok(());
        }
        self.restart_record(id)?;
        self.schedule();
        Ok(())
    }

    /// Clears the progress and the part file, then queues the download again.
    pub(super) fn restart_record(&self, id: DownloadId) -> Result<()> {
        let record = self.load(id)?;
        self.shared.store.clear_progress(id)?;
        remove_if_exists(&record.part_path())?;
        self.set_status(id, DownloadStatus::Queued, None)
    }

    pub(super) fn remove(&mut self, id: DownloadId, delete_file: bool) -> Result<()> {
        let record = self.load(id)?;
        if self.active.contains_key(&id) {
            self.removing.insert(id, delete_file);
            self.cancel_task(id);
            return Ok(());
        }
        self.remove_record(&record, delete_file, false)
    }

    /// Deletes the files that belong to a download and its stored record.
    /// `finished` tells that its transfer completed just now.
    pub(super) fn remove_record(
        &self,
        record: &DownloadRecord,
        delete_file: bool,
        finished: bool,
    ) -> Result<()> {
        let completed = finished || record.status == DownloadStatus::Completed;
        if !completed {
            remove_if_exists(&record.part_path())?;
        }
        if delete_file && completed {
            remove_if_exists(&record.final_path())?;
        }
        self.shared.store.delete_download(record.id)?;
        self.emit(EngineEvent::Removed(record.id));
        Ok(())
    }

    pub(super) fn set_connections(&mut self, id: DownloadId, connections: u8) -> Result<()> {
        self.load(id)?;
        let connections = connections.clamp(1, MAX_CONNECTIONS);
        self.shared.store.set_connections(id, connections)?;
        if let Some(task) = self.active.get(&id) {
            task.live.set_target(u32::from(connections));
        }
        Ok(())
    }

    pub(super) fn set_speed_limit(&mut self, id: DownloadId, limit: Option<u64>) -> Result<()> {
        self.load(id)?;
        self.shared.store.set_speed_limit(id, limit)?;
        if let Some(task) = self.active.get(&id) {
            task.limiter.set_rate(limit.unwrap_or(0));
        }
        Ok(())
    }

    pub(super) fn update_settings(&mut self, settings: EngineSettings) -> Result<()> {
        let mut settings = settings.sanitized();
        settings.speed_schedule = settings.speed_schedule.sanitized();
        let client = build_client(&settings)?;
        self.shared.replace_settings(settings, client);
        self.apply_speed_limit();
        self.schedule();
        Ok(())
    }

    /// Sets the global limiter to the limit of the current time. The rate changes only when it differs.
    pub(super) fn apply_speed_limit(&self) {
        self.apply_speed_limit_at(Local::now().naive_local());
    }

    pub(super) fn apply_speed_limit_at(&self, now: NaiveDateTime) {
        let settings = self.shared.settings();
        let rate = settings
            .speed_schedule
            .limit_at(settings.global_speed_limit, now)
            .unwrap_or(0);
        if self.applied_rate.get() != Some(rate) {
            self.global_limiter.set_rate(rate);
            self.applied_rate.set(Some(rate));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;
    use crate::store::Store;
    use std::sync::RwLock;
    use tokio::sync::mpsc;

    fn row(file_name: &str, status: DownloadStatus) -> NewDownload {
        NewDownload {
            url: "https://example.com/file".into(),
            final_url: None,
            referrer: None,
            file_name: file_name.into(),
            save_dir: "C:/unused".into(),
            category: Category::Other,
            status,
            total_size: None,
            resumable: false,
            connections: 1,
            speed_limit: None,
            etag: None,
            last_modified: None,
            content_type: None,
            headers: Vec::new(),
            queue_position: 1,
            created_at: 0,
            expected_sha256: None,
            kind: crate::model::DownloadKind::File,
            mirrors: Vec::new(),
            options: None,
        }
    }

    /// An actor whose loop is not running, so its handlers can be called directly.
    /// No handler under test starts a transfer, so no engine task is spawned.
    fn test_actor() -> (ManagerActor, Store) {
        let store = Store::open_in_memory().expect("store");
        let settings = EngineSettings::default();
        let client = build_client(&settings).expect("client");
        let shared = Arc::new(Shared {
            store: store.clone(),
            settings: RwLock::new(settings),
            client: RwLock::new(client),
        });
        let (events, _events_rx) = mpsc::unbounded_channel();
        let (self_tx, _cmd_rx) = mpsc::unbounded_channel();
        let limiter = Arc::new(RateLimiter::new(0));
        (ManagerActor::new(shared, events, self_tx, limiter), store)
    }

    #[tokio::test]
    async fn pause_moves_a_queued_download_to_paused() {
        let (mut actor, store) = test_actor();
        let record = store
            .insert_download(&row("queued.bin", DownloadStatus::Queued))
            .expect("insert");
        actor.pause(record.id).expect("pause");
        let after = store.get_download(record.id).expect("read").expect("row");
        assert_eq!(after.status, DownloadStatus::Paused);
    }

    #[tokio::test]
    async fn resume_leaves_a_completed_download_alone() {
        let (mut actor, store) = test_actor();
        let record = store
            .insert_download(&row("done.bin", DownloadStatus::Completed))
            .expect("insert");
        actor.resume(record.id).expect("resume");
        let after = store.get_download(record.id).expect("read").expect("row");
        assert_eq!(after.status, DownloadStatus::Completed);
    }

    #[tokio::test]
    async fn speed_schedule_updates_the_global_limiter_at_each_check() {
        let (mut actor, _store) = test_actor();
        let limiter = Arc::clone(&actor.global_limiter);
        let settings: EngineSettings = serde_json::from_value(serde_json::json!({
            "speedSchedule": {
                "enabled": true,
                "rules": [{
                    "days": [0, 1, 2, 3, 4, 5, 6],
                    "start": "10:00",
                    "end": "11:00",
                    "limit": 4096
                }]
            }
        }))
        .expect("settings");
        actor.update_settings(settings).expect("update");
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).expect("date");
        actor.apply_speed_limit_at(day.and_hms_opt(10, 30, 0).expect("time"));
        assert_eq!(limiter.rate(), 4096);
        actor.apply_speed_limit_at(day.and_hms_opt(12, 0, 0).expect("time"));
        assert_eq!(limiter.rate(), 0);
    }

    #[tokio::test]
    async fn settings_update_reaches_the_global_limiter() {
        let (mut actor, _store) = test_actor();
        let limiter = Arc::clone(&actor.global_limiter);
        let capped = EngineSettings {
            global_speed_limit: Some(2048),
            ..EngineSettings::default()
        };
        actor.update_settings(capped).expect("update");
        assert_eq!(limiter.rate(), 2048);
        assert_eq!(actor.shared.settings().global_speed_limit, Some(2048));
        actor
            .update_settings(EngineSettings::default())
            .expect("update");
        assert_eq!(limiter.rate(), 0);
    }
}
