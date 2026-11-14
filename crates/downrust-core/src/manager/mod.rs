// SPDX-License-Identifier: GPL-3.0-or-later
//! Download manager: the public handle to the scheduling actor.
mod active;
mod actor;
mod command;
mod edits;
mod idle;
mod naming;
mod queue;
mod scheduler;
mod stats;

use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use tokio::sync::{
    mpsc::{self, UnboundedSender},
    oneshot,
};

use actor::ManagerActor;
use command::{request, Cmd, ResolvedAdd};
use naming::{media_file_name, media_name_with_extension, resolve_target};

use crate::{
    engine::{checksum::hash_file, limiter::RateLimiter},
    error::{Error, Result},
    http::client::build_client,
    model::{
        AddRequest, ChecksumReport, ChecksumState, DownloadId, DownloadKind, DownloadRecord,
        DownloadStatus, EngineEvent, HeaderPair, ProbeInfo, QueueMove, TargetSuggestion,
    },
    settings::EngineSettings,
    store::Store,
    util::{digest::normalize_sha256, filename::file_name_from_url},
};

const FALLBACK_NAME: &str = "download";
const SHA256_MESSAGE: &str = "The SHA-256 must be 64 hexadecimal characters.";

/// Checks an optional SHA-256 value. Blank text counts as no value.
fn normalize_expected(value: Option<String>) -> Result<Option<String>> {
    match value {
        Some(text) if !text.trim().is_empty() => normalize_sha256(&text)
            .map(Some)
            .ok_or_else(|| Error::InvalidInput(SHA256_MESSAGE.to_owned())),
        _ => Ok(None),
    }
}

/// State shared by the handle and the actor.
struct Shared {
    store: Store,
    settings: RwLock<EngineSettings>,
    client: RwLock<reqwest::Client>,
}

impl Shared {
    fn settings(&self) -> EngineSettings {
        self.settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn client(&self) -> reqwest::Client {
        self.client
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn replace_settings(&self, settings: EngineSettings, client: reqwest::Client) {
        *self
            .settings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = settings;
        *self.client.write().unwrap_or_else(PoisonError::into_inner) = client;
    }
}

/// Name of a download that the engine does not fetch over HTTP, before the transfer finds the
/// real one.
fn external_initial_name(address: &str, kind: DownloadKind) -> String {
    match kind {
        DownloadKind::Site => crate::engine::site::initial_name(address),
        DownloadKind::Torrent => crate::engine::torrent::initial_name(address),
        DownloadKind::Ftp => crate::engine::ftp::initial_name(address),
        _ => fallback_name(address),
    }
}

/// Name used when neither the request nor a probe gives one.
fn fallback_name(address: &str) -> String {
    url::Url::parse(address.trim())
        .ok()
        .and_then(|parsed| file_name_from_url(&parsed))
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

#[derive(Clone)]
pub struct DownloadManager {
    tx: UnboundedSender<Cmd>,
    shared: Arc<Shared>,
}

impl DownloadManager {
    pub async fn start(
        store: Store,
        settings: EngineSettings,
        events: UnboundedSender<EngineEvent>,
    ) -> Result<Self> {
        let settings = settings.sanitized();
        let on_start = if settings.resume_on_startup {
            DownloadStatus::Queued
        } else {
            DownloadStatus::Paused
        };
        store.recover_interrupted(on_start)?;
        let client = build_client(&settings)?;
        let global_limiter = Arc::new(RateLimiter::new(settings.global_speed_limit.unwrap_or(0)));
        let shared = Arc::new(Shared {
            store,
            settings: RwLock::new(settings),
            client: RwLock::new(client),
        });
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = ManagerActor::new(Arc::clone(&shared), events, tx.clone(), global_limiter);
        tokio::spawn(actor.run(rx));
        tx.send(Cmd::Reschedule)
            .map_err(|_| Error::ManagerStopped)?;
        Ok(Self { tx, shared })
    }

    pub async fn probe(&self, url: &str, headers: &[HeaderPair]) -> Result<ProbeInfo> {
        let settings = self.shared.settings();
        let timeout = Duration::from_secs(settings.read_timeout_secs);
        let client = self.shared.client();
        let headers = crate::auth::with_site_login(&settings, url, headers);
        crate::http::probe::probe(&client, url, &headers, timeout).await
    }

    pub fn suggest_target(&self, file_name: &str) -> Result<TargetSuggestion> {
        let settings = self.shared.settings();
        resolve_target(&self.shared.store, &settings, file_name, None)
    }

    /// Adds a download. Without a file name, the address is probed to find one.
    pub async fn add(&self, mut req: AddRequest) -> Result<DownloadRecord> {
        req.expected_sha256 = normalize_expected(req.expected_sha256.take())?;
        let mut probe = req.probe.take();
        let kind = match req.kind {
            Some(kind) => kind,
            None => match DownloadKind::from_url(&req.url) {
                Some(kind) => kind,
                None => {
                    if probe.is_none() && req.file_name.is_none() {
                        probe = self.probe(&req.url, &req.headers).await.ok();
                    }
                    probe
                        .as_ref()
                        .and_then(|p| p.content_type.as_deref())
                        .and_then(DownloadKind::from_content_type)
                        .unwrap_or_default()
                }
            },
        };
        req.kind = Some(kind);
        let (file_name, probe) = if kind.is_external() {
            let name = req
                .file_name
                .take()
                .unwrap_or_else(|| external_initial_name(&req.url, kind));
            (name, None)
        } else if kind.is_media() {
            let convert = self.settings().media.convert_to_mp4;
            let name = match req.file_name.take() {
                Some(name) => media_name_with_extension(&name, kind, convert),
                None => media_file_name(&req.url, kind, convert),
            };
            (name, None)
        } else {
            match (req.file_name.take(), probe) {
                (Some(name), probe) => (name, probe),
                (None, Some(probe)) => (probe.file_name.clone(), Some(probe)),
                (None, None) => match self.probe(&req.url, &req.headers).await {
                    Ok(probe) => (probe.file_name.clone(), Some(probe)),
                    Err(_) => (fallback_name(&req.url), None),
                },
            }
        };
        request(&self.tx, |reply| Cmd::Add {
            req: Box::new(ResolvedAdd {
                req,
                file_name,
                probe,
            }),
            reply,
        })
        .await
    }

    /// Live progress merged in.
    pub async fn list(&self) -> Result<Vec<DownloadRecord>> {
        request(&self.tx, |reply| Cmd::List { reply }).await
    }

    pub async fn get(&self, id: DownloadId) -> Result<DownloadRecord> {
        request(&self.tx, |reply| Cmd::Get { id, reply }).await
    }

    pub async fn pause(&self, id: DownloadId) -> Result<()> {
        request(&self.tx, |reply| Cmd::Pause { id, reply }).await
    }

    pub async fn resume(&self, id: DownloadId) -> Result<()> {
        request(&self.tx, |reply| Cmd::Resume { id, reply }).await
    }

    pub async fn restart(&self, id: DownloadId) -> Result<()> {
        request(&self.tx, |reply| Cmd::Restart { id, reply }).await
    }

    pub async fn remove(&self, id: DownloadId, delete_file: bool) -> Result<()> {
        request(&self.tx, |reply| Cmd::Remove {
            id,
            delete_file,
            reply,
        })
        .await
    }

    pub async fn pause_all(&self) -> Result<()> {
        request(&self.tx, |reply| Cmd::PauseAll { reply }).await
    }

    pub async fn resume_all(&self) -> Result<()> {
        request(&self.tx, |reply| Cmd::ResumeAll { reply }).await
    }

    pub async fn set_connections(&self, id: DownloadId, connections: u8) -> Result<()> {
        request(&self.tx, |reply| Cmd::SetConnections {
            id,
            connections,
            reply,
        })
        .await
    }

    pub async fn set_speed_limit(&self, id: DownloadId, limit: Option<u64>) -> Result<()> {
        request(&self.tx, |reply| Cmd::SetSpeedLimit { id, limit, reply }).await
    }

    pub async fn update_settings(&self, settings: EngineSettings) -> Result<()> {
        request(&self.tx, |reply| Cmd::UpdateSettings {
            settings: Box::new(settings),
            reply,
        })
        .await
    }

    /// Moves a queued download up, down, to the top or to the bottom of the queue.
    pub async fn move_in_queue(&self, id: DownloadId, to: QueueMove) -> Result<()> {
        request(&self.tx, |reply| Cmd::MoveInQueue { id, to, reply }).await
    }

    /// Hashes a completed download and compares it with `expected`. The stored expected value and
    /// the result are kept on the download.
    pub async fn verify_checksum(
        &self,
        id: DownloadId,
        expected: String,
    ) -> Result<ChecksumReport> {
        let expected = normalize_sha256(&expected)
            .ok_or_else(|| Error::InvalidInput(SHA256_MESSAGE.to_owned()))?;
        let record = self.get(id).await?;
        if record.status != DownloadStatus::Completed {
            return Err(Error::InvalidInput(
                "Only completed downloads can be verified.".to_owned(),
            ));
        }
        let path = record.final_path();
        let actual = tokio::task::spawn_blocking(move || hash_file(&path))
            .await
            .map_err(|err| Error::Transfer(format!("checksum task stopped: {err}")))??;
        let matches = actual == expected;
        let state = if matches {
            ChecksumState::Ok
        } else {
            ChecksumState::Mismatch
        };
        request(&self.tx, |reply| Cmd::RecordChecksum {
            id,
            expected,
            state,
            reply,
        })
        .await?;
        Ok(ChecksumReport { actual, matches })
    }

    /// Points a paused, failed or queued download at a new address. The new address must lead to
    /// a file of the same size, so saved progress stays valid.
    pub async fn refresh_address(&self, id: DownloadId, url: String) -> Result<DownloadRecord> {
        let url = url.trim().to_owned();
        let record = self.get(id).await?;
        if !edits::address_change_applies(record.status) {
            return Err(Error::InvalidInput(
                "The address can only be changed for a paused, failed or queued download."
                    .to_owned(),
            ));
        }
        let headers = record.request_headers();
        let probe = self.probe(&url, &headers).await?;
        request(&self.tx, |reply| Cmd::UpdateAddress {
            id,
            url,
            probe: Box::new(probe),
            reply,
        })
        .await
    }

    pub fn settings(&self) -> EngineSettings {
        self.shared.settings()
    }

    /// Cancels running transfers and waits until they stop. Calling it again after the actor
    /// has stopped succeeds at once.
    pub async fn shutdown(&self) -> Result<()> {
        let (reply, done) = oneshot::channel();
        if self.tx.send(Cmd::Shutdown { reply }).is_err() {
            return Ok(());
        }
        let _ = done.await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_list_and_shutdown_on_an_empty_store() {
        let store = Store::open_in_memory().expect("store");
        let (events, _events_rx) = mpsc::unbounded_channel();
        let manager = DownloadManager::start(store, EngineSettings::default(), events)
            .await
            .expect("start");
        assert!(manager.list().await.expect("list").is_empty());
        manager.shutdown().await.expect("shutdown");
        assert!(matches!(manager.list().await, Err(Error::ManagerStopped)));
        manager.shutdown().await.expect("second shutdown");
    }

    #[test]
    fn fallback_name_uses_the_address_or_a_default() {
        assert_eq!(fallback_name("https://example.com/a/data.bin"), "data.bin");
        assert_eq!(fallback_name("https://example.com/"), FALLBACK_NAME);
        assert_eq!(fallback_name("not a url"), FALLBACK_NAME);
    }
}
