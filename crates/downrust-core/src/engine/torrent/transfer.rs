// SPDX-License-Identifier: GPL-3.0-or-later
//! Runs one torrent: adds it to the session, reports progress and finishes it.
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::limits::LimitsConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, TorrentStats, TorrentStatsState,
};

use super::session::shared;
use super::source::{detect, Source};
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::http::request::get_request;
use crate::model::{DownloadRecord, HeaderPair, Phase, SegmentView};

const TICK: Duration = Duration::from_millis(500);
const MAX_TORRENT_FILE: usize = 10 * 1024 * 1024;

/// Runs a torrent download to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    let record = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    let session = shared(&ctx.settings.torrent, PathBuf::from(&record.save_dir)).await?;
    download(ctx, &session, &record, Vec::new()).await
}

/// Runs the download in the given session. `initial_peers` are extra peers to try first.
pub(crate) async fn download(
    ctx: &TaskContext,
    session: &Arc<Session>,
    record: &DownloadRecord,
    initial_peers: Vec<SocketAddr>,
) -> Result<TaskOutcome> {
    ctx.live.set_phase(Phase::Connecting);
    let source = tokio::select! {
        _ = ctx.cancel.cancelled() => return Ok(TaskOutcome::Cancelled),
        loaded = load_source(ctx, record) => loaded?,
    };
    let options = AddTorrentOptions {
        output_folder: Some(record.save_dir.clone()),
        // Lets a restarted torrent check and reuse the files already on disk.
        overwrite: true,
        ratelimits: limits(ctx, record),
        initial_peers: Some(initial_peers),
        ..AddTorrentOptions::default()
    };
    let handle = match session
        .add_torrent(source, Some(options))
        .await
        .map_err(transfer_error)?
    {
        AddTorrentResponse::Added(_, handle) => handle,
        AddTorrentResponse::AlreadyManaged(_, handle) => {
            if handle.is_paused() {
                session.unpause(&handle).await.map_err(transfer_error)?;
            }
            handle
        }
        AddTorrentResponse::ListOnly(_) => {
            return Err(Error::Transfer("the torrent was only listed".to_owned()));
        }
    };

    let mut named = false;
    loop {
        let stats = handle.stats();
        if let TorrentStatsState::Error = stats.state {
            let message = stats
                .error
                .clone()
                .unwrap_or_else(|| "the torrent failed".to_owned());
            let _ = session
                .delete(TorrentIdOrHash::Id(handle.id()), false)
                .await;
            return Err(Error::Transfer(message));
        }
        if !named {
            if let Some(name) = handle.name() {
                ctx.store
                    .set_name_and_size(ctx.id, &name, stats.total_bytes)?;
                named = true;
            }
        }
        if stats.finished {
            break;
        }
        report(ctx, &stats);
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                session.pause(&handle).await.map_err(transfer_error)?;
                return Ok(TaskOutcome::Cancelled);
            }
            _ = tokio::time::sleep(TICK) => {}
        }
    }

    ctx.live.set_phase(Phase::Finalizing);
    let name = handle.name().unwrap_or_else(|| record.file_name.clone());
    let total = handle.stats().total_bytes;
    if !ctx.settings.torrent.seed_after_finish {
        session
            .delete(TorrentIdOrHash::Id(handle.id()), false)
            .await
            .map_err(transfer_error)?;
    }
    ctx.store.record_finished_file(ctx.id, &name, total)?;
    Ok(TaskOutcome::Completed)
}

/// Reads the torrent description: a magnet link as is, a .torrent over HTTP(S), or a local file.
async fn load_source(ctx: &TaskContext, record: &DownloadRecord) -> Result<AddTorrent<'static>> {
    match detect(&record.url)? {
        Source::Magnet(link) => Ok(AddTorrent::from_url(link)),
        Source::Http(url) => {
            let bytes = fetch_torrent_file(ctx, &url, &record.request_headers()).await?;
            Ok(AddTorrent::from_bytes(bytes))
        }
        Source::File(path) => Ok(AddTorrent::from_bytes(std::fs::read(&path)?)),
    }
}

async fn fetch_torrent_file(
    ctx: &TaskContext,
    url: &str,
    headers: &[HeaderPair],
) -> Result<Vec<u8>> {
    let mut response = get_request(&ctx.client, url, headers, None, None)
        .send()
        .await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::HttpStatus(status));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_TORRENT_FILE as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_TORRENT_FILE {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn too_large() -> Error {
    Error::Transfer("the .torrent file is larger than 10 MB".to_owned())
}

/// Per-torrent rate limits: the smaller of the download's own limit and the global limit,
/// and the upload limit from the torrent settings. They apply when the torrent is added.
fn limits(ctx: &TaskContext, record: &DownloadRecord) -> LimitsConfig {
    let own = record.speed_limit.filter(|limit| *limit > 0);
    let global = ctx.settings.global_speed_limit.filter(|limit| *limit > 0);
    let download = match (own, global) {
        (Some(own), Some(global)) => Some(own.min(global)),
        (own, global) => own.or(global),
    };
    LimitsConfig {
        upload_bps: to_bps(ctx.settings.torrent.upload_limit),
        download_bps: to_bps(download),
    }
}

fn to_bps(limit: Option<u64>) -> Option<NonZeroU32> {
    limit.and_then(|bytes| NonZeroU32::new(u32::try_from(bytes).unwrap_or(u32::MAX)))
}

/// Copies the torrent stats into the live statistics used by the progress view.
fn report(ctx: &TaskContext, stats: &TorrentStats) {
    let peers = stats
        .live
        .as_ref()
        .map_or(0, |live| live.snapshot.peer_stats.live);
    let phase = if stats.live.is_some() {
        Phase::Downloading
    } else {
        Phase::Connecting
    };
    ctx.live.set_phase(phase);
    ctx.live.set_connections(peers);
    let total = stats.total_bytes;
    let done = stats.progress_bytes.min(total);
    ctx.live
        .set_media_total(Some(total).filter(|total| *total > 0));
    ctx.live.set_media_done(stats.progress_bytes);
    let views = if total > 0 {
        vec![SegmentView {
            start: 0,
            end: total,
            done,
            active: true,
        }]
    } else {
        Vec::new()
    };
    ctx.live.set_media_views(views);
}

fn transfer_error(err: impl std::fmt::Display) -> Error {
    Error::Transfer(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use librqbit::spawn_utils::BlockingSpawner;
    use librqbit::{create_torrent, CreateTorrentOptions, ListenerOptions, SessionOptions};
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::engine::limiter::RateLimiter;
    use crate::engine::live::LiveStats;
    use crate::http::client::build_client;
    use crate::model::{Category, DownloadKind, DownloadStatus, NewDownload};
    use crate::settings::EngineSettings;
    use crate::store::Store;

    /// Offline session: no DHT, no trackers, no UPnP, listens on loopback only.
    fn offline_options() -> SessionOptions {
        SessionOptions {
            dht: None,
            persistence: None,
            fastresume: false,
            disable_trackers: true,
            disable_local_service_discovery: true,
            listen: Some(ListenerOptions {
                listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                enable_upnp_port_forwarding: false,
                ipv4_only: true,
                ..ListenerOptions::default()
            }),
            ..SessionOptions::default()
        }
    }

    #[test]
    fn zero_limits_mean_unlimited() {
        assert_eq!(to_bps(None), None);
        assert_eq!(to_bps(Some(0)), None);
        assert_eq!(to_bps(Some(2048)).map(NonZeroU32::get), Some(2048));
        assert_eq!(to_bps(Some(u64::MAX)).map(NonZeroU32::get), Some(u32::MAX));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn torrent_downloads_from_a_local_seed() {
        let root = tempfile::tempdir().expect("temp dir");
        let seed_dir = root.path().join("seed");
        let leech_dir = root.path().join("leech");
        std::fs::create_dir_all(&seed_dir).expect("seed dir");
        std::fs::create_dir_all(&leech_dir).expect("leech dir");
        let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(seed_dir.join("sample.bin"), &payload).expect("payload written");

        let created = create_torrent(
            &seed_dir.join("sample.bin"),
            CreateTorrentOptions {
                name: None,
                trackers: Vec::new(),
                piece_length: Some(32 * 1024),
            },
            &BlockingSpawner::new(4),
        )
        .await
        .expect("torrent created");
        let torrent_bytes = created.as_bytes().expect("torrent bytes");
        let torrent_path = root.path().join("sample.torrent");
        std::fs::write(&torrent_path, &torrent_bytes[..]).expect("torrent file written");

        let seeder = Session::new_with_opts(seed_dir.clone(), offline_options())
            .await
            .expect("seeder starts");
        let seed_added = seeder
            .add_torrent(
                AddTorrent::from_bytes(torrent_bytes),
                Some(AddTorrentOptions {
                    output_folder: Some(seed_dir.display().to_string()),
                    overwrite: true,
                    ..AddTorrentOptions::default()
                }),
            )
            .await
            .expect("seed added");
        let seed_handle = seed_added.into_handle().expect("seed handle");
        tokio::time::timeout(Duration::from_secs(30), seed_handle.wait_until_completed())
            .await
            .expect("seed is ready in time")
            .expect("seed check passes");
        let seed_addr = seeder.listen_addr().expect("seeder listens");

        let settings = EngineSettings {
            default_dir: leech_dir.display().to_string(),
            ..EngineSettings::default()
        };
        let store = Store::open_in_memory().expect("store opens");
        let record = store
            .insert_download(&NewDownload {
                expected_sha256: None,
                kind: DownloadKind::Torrent,
                mirrors: Vec::new(),
                options: None,
                url: torrent_path.display().to_string(),
                final_url: None,
                referrer: None,
                file_name: "sample".to_owned(),
                save_dir: leech_dir.display().to_string(),
                category: Category::Other,
                status: DownloadStatus::Queued,
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
            })
            .expect("row inserted");
        let leecher = Session::new_with_opts(leech_dir.clone(), offline_options())
            .await
            .expect("leecher starts");
        let (events, _rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = TaskContext {
            id: record.id,
            store: store.clone(),
            client: build_client(&settings).expect("client builds"),
            settings,
            global_limiter: Arc::new(RateLimiter::new(0)),
            limiter: Arc::new(RateLimiter::new(0)),
            live: Arc::new(LiveStats::new(1)),
            cancel: CancellationToken::new(),
            events,
        };

        let outcome = tokio::time::timeout(
            Duration::from_secs(60),
            download(&ctx, &leecher, &record, vec![seed_addr]),
        )
        .await
        .expect("download finishes in time")
        .expect("download runs");

        assert_eq!(outcome, TaskOutcome::Completed);
        let saved = std::fs::read(leech_dir.join("sample.bin")).expect("downloaded file");
        assert_eq!(saved.len(), payload.len());
        assert!(saved == payload, "downloaded bytes differ from the source");
        let stored = store
            .get_download(record.id)
            .expect("row read")
            .expect("row exists");
        assert_eq!(stored.file_name, "sample.bin");
        assert_eq!(stored.total_size, Some(payload.len() as u64));
    }
}
