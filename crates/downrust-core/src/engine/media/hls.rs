// SPDX-License-Identifier: GPL-3.0-or-later
//! HLS downloads: picks a variant, fetches its parts in parallel with resume, decrypts them, joins
//! them in order and, when needed, converts or muxes the result with ffmpeg.
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{stream, TryStreamExt};
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

use super::crypto::{decrypt_segment, sequence_iv};
use super::fetch::{fetch, write_part};
use super::ffmpeg::{find_ffmpeg, mux_to_mp4, remux_to_mp4, CANCELLED, NEEDS_FFMPEG};
use super::playlist::{
    parse, AudioRendition, MasterPlaylist, MediaPlaylist, Playlist, Segment, Variant,
};
use super::state::{Stream, Tracker};
use crate::engine::file::finalize;
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::model::{DownloadRecord, HeaderPair, Phase};
use crate::settings::MediaQuality;

/// Most parts downloaded at the same time.
pub(crate) const MAX_PARTS: u8 = 16;
const LIVE_MESSAGE: &str = "Live streams are not supported yet.";

type KeyCache = AsyncMutex<HashMap<String, [u8; 16]>>;

/// Runs an HLS download to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    match run_inner(ctx).await {
        Err(Error::Transfer(message)) if message == CANCELLED => Ok(TaskOutcome::Cancelled),
        other => other,
    }
}

async fn run_inner(ctx: &TaskContext) -> Result<TaskOutcome> {
    let record = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    let headers = record.request_headers();
    let ffmpeg = find_ffmpeg(&ctx.settings.media);
    let first = fetch(ctx, &record.url, &headers, None).await?;
    let base = Url::parse(&first.url).map_err(|err| Error::InvalidUrl(err.to_string()))?;
    let (video, audio) = match parse(&String::from_utf8_lossy(&first.bytes), &base)? {
        Playlist::Master(master) => select_streams(ctx, &headers, &master).await?,
        Playlist::Media(media) => (checked(media)?, None),
    };
    if audio.is_some() && ffmpeg.is_none() {
        return Err(Error::Transfer(NEEDS_FFMPEG.to_owned()));
    }
    let media_dir = media_dir(&record);
    std::fs::create_dir_all(&media_dir)?;
    let workers = usize::from(record.connections.clamp(1, MAX_PARTS));
    ctx.live.set_phase(Phase::Downloading);
    ctx.live
        .set_connections(u32::try_from(workers).unwrap_or(1));

    let video_stream =
        download_stream(ctx, &headers, &video, &media_dir.join("video"), workers, 0).await?;
    let base_bytes = video_stream.bytes;
    let audio_stream = match &audio {
        Some(media) => Some(
            download_stream(
                ctx,
                &headers,
                media,
                &media_dir.join("audio"),
                workers,
                base_bytes,
            )
            .await?,
        ),
        None => None,
    };
    ctx.live.set_phase(Phase::Finalizing);
    let output = Output {
        ctx,
        record: &record,
        media_dir: &media_dir,
        ffmpeg: ffmpeg.as_deref(),
    };
    output.finish(&video_stream, audio_stream.as_ref()).await?;
    Ok(TaskOutcome::Completed)
}

/// Picks the video variant by quality and the audio rendition of its group.
async fn select_streams(
    ctx: &TaskContext,
    headers: &[HeaderPair],
    master: &MasterPlaylist,
) -> Result<(MediaPlaylist, Option<MediaPlaylist>)> {
    let variant = choose_variant(&master.variants, ctx.settings.media.quality)
        .ok_or_else(|| Error::Protocol("the master playlist lists no variants".to_owned()))?;
    let video = load_media(ctx, headers, &variant.url).await?;
    let audio = match variant
        .audio_group
        .as_deref()
        .and_then(|group| audio_url(&master.audio, group))
    {
        Some(url) => Some(load_media(ctx, headers, &url).await?),
        None => None,
    };
    Ok((video, audio))
}

/// Highest BANDWIDTH for `Best`, lowest for `Worst`.
pub(crate) fn choose_variant(variants: &[Variant], quality: MediaQuality) -> Option<&Variant> {
    match quality {
        MediaQuality::Best => variants.iter().max_by_key(|v| v.bandwidth),
        MediaQuality::Worst => variants.iter().min_by_key(|v| v.bandwidth),
    }
}

/// The URI of the default audio rendition of a group, or of the first one without a default.
pub(crate) fn audio_url(renditions: &[AudioRendition], group: &str) -> Option<String> {
    let candidates: Vec<&AudioRendition> = renditions
        .iter()
        .filter(|r| r.group_id == group && r.url.is_some())
        .collect();
    candidates
        .iter()
        .find(|r| r.is_default)
        .or(candidates.first())
        .and_then(|r| r.url.clone())
}

async fn load_media(ctx: &TaskContext, headers: &[HeaderPair], url: &str) -> Result<MediaPlaylist> {
    let fetched = fetch(ctx, url, headers, None).await?;
    let base = Url::parse(&fetched.url).map_err(|err| Error::InvalidUrl(err.to_string()))?;
    match parse(&String::from_utf8_lossy(&fetched.bytes), &base)? {
        Playlist::Media(media) => checked(media),
        Playlist::Master(_) => Err(Error::Protocol(
            "nested master playlists are not supported".to_owned(),
        )),
    }
}

/// A media playlist must list segments and end. Without an end marker it is a live stream.
fn checked(media: MediaPlaylist) -> Result<MediaPlaylist> {
    if !media.ended {
        return Err(Error::Transfer(LIVE_MESSAGE.to_owned()));
    }
    if media.segments.is_empty() {
        return Err(Error::Protocol("the playlist lists no segments".to_owned()));
    }
    Ok(media)
}

/// Folder that holds the parts, next to the output file.
pub(crate) fn media_dir(record: &DownloadRecord) -> PathBuf {
    Path::new(&record.save_dir).join(format!("{}.downrust-media", record.file_name))
}

/// Downloads the init section and all parts of one stream. Parts that exist already are kept.
async fn download_stream(
    ctx: &TaskContext,
    headers: &[HeaderPair],
    media: &MediaPlaylist,
    dir: &Path,
    workers: usize,
    base_bytes: u64,
) -> Result<Stream> {
    std::fs::create_dir_all(dir)?;
    let init = match &media.map {
        Some(map) => {
            let path = dir.join("init");
            if !path.exists() {
                let fetched = fetch(ctx, &map.url, headers, map.byterange).await?;
                write_part(path.clone(), fetched.bytes).await?;
            }
            Some(path)
        }
        None => None,
    };
    let paths: Vec<PathBuf> = (0..media.segments.len())
        .map(|index| dir.join(format!("seg-{index:06}")))
        .collect();
    let tracker = Arc::new(Tracker::new(Arc::clone(&ctx.live), paths.len(), base_bytes));
    let mut todo = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        match std::fs::metadata(path) {
            Ok(meta) => {
                ctx.live.add_media_bytes(meta.len());
                tracker.finish(index, meta.len());
            }
            Err(_) => todo.push(index),
        }
    }
    let keys: KeyCache = AsyncMutex::new(HashMap::new());
    let keys = &keys;
    let tracker_ref = &tracker;
    stream::iter(todo.into_iter().map(Ok::<usize, Error>))
        .try_for_each_concurrent(workers, |index| {
            let path = paths[index].clone();
            let segment = &media.segments[index];
            async move {
                tracker_ref.start(index);
                let plain = download_part(ctx, headers, segment, keys).await?;
                let len = plain.len() as u64;
                write_part(path, plain).await?;
                tracker_ref.finish(index, len);
                Ok(())
            }
        })
        .await?;
    let bytes = tracker.total_bytes();
    Ok(Stream {
        init,
        parts: paths,
        fmp4: media.map.is_some(),
        bytes,
    })
}

/// Fetches one part and decrypts it when it has an AES-128 key.
async fn download_part(
    ctx: &TaskContext,
    headers: &[HeaderPair],
    segment: &Segment,
    keys: &KeyCache,
) -> Result<Vec<u8>> {
    let fetched = fetch(ctx, &segment.url, headers, segment.byterange).await?;
    match &segment.key {
        None => Ok(fetched.bytes),
        Some(key) => {
            let secret = key_for(ctx, headers, &key.url, keys).await?;
            let iv = key.iv.unwrap_or_else(|| sequence_iv(segment.sequence));
            decrypt_segment(&secret, &iv, &fetched.bytes)
        }
    }
}

/// Reads an AES-128 key once per address.
async fn key_for(
    ctx: &TaskContext,
    headers: &[HeaderPair],
    url: &str,
    keys: &KeyCache,
) -> Result<[u8; 16]> {
    let mut cache = keys.lock().await;
    if let Some(key) = cache.get(url) {
        return Ok(*key);
    }
    let fetched = fetch(ctx, url, headers, None).await?;
    let key = <[u8; 16]>::try_from(fetched.bytes.as_slice())
        .map_err(|_| Error::Protocol("the encryption key must be 16 bytes".to_owned()))?;
    cache.insert(url.to_owned(), key);
    Ok(key)
}

/// Joins the init section and the parts of a stream into one file.
pub(crate) async fn join(stream: &Stream, out: &Path) -> Result<()> {
    let parts: Vec<PathBuf> = stream
        .init
        .iter()
        .cloned()
        .chain(stream.parts.iter().cloned())
        .collect();
    let out = out.to_path_buf();
    tokio::task::spawn_blocking(move || -> io::Result<()> {
        let mut writer = BufWriter::new(File::create(&out)?);
        for part in &parts {
            let mut reader = File::open(part)?;
            io::copy(&mut reader, &mut writer)?;
        }
        writer.flush()
    })
    .await
    .map_err(|err| Error::Transfer(format!("join task stopped: {err}")))??;
    Ok(())
}

/// What happens after the parts are downloaded: join, mux or convert, name, record.
struct Output<'a> {
    ctx: &'a TaskContext,
    record: &'a DownloadRecord,
    media_dir: &'a Path,
    ffmpeg: Option<&'a Path>,
}

impl Output<'_> {
    async fn finish(&self, video: &Stream, audio: Option<&Stream>) -> Result<()> {
        let joined_video = self.media_dir.join("video.joined");
        join(video, &joined_video).await?;
        let (source, extension) = self.combine(joined_video, video.fmp4, audio).await?;
        let save_dir = PathBuf::from(&self.record.save_dir);
        let stem = Path::new(&self.record.file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("video")
            .to_owned();
        let final_name = finalize(&source, &save_dir, &format!("{stem}.{extension}")).await?;
        let size = std::fs::metadata(save_dir.join(&final_name))?.len();
        self.ctx
            .store
            .record_finished_file(self.ctx.id, &final_name, size)?;
        if let Err(err) = std::fs::remove_dir_all(self.media_dir) {
            tracing::warn!("cannot remove the part folder: {err}");
        }
        Ok(())
    }

    /// Returns the file to name and its extension.
    async fn combine(
        &self,
        joined_video: PathBuf,
        video_is_mp4: bool,
        audio: Option<&Stream>,
    ) -> Result<(PathBuf, &'static str)> {
        let cancel = &self.ctx.cancel;
        if let Some(audio) = audio {
            let program = self
                .ffmpeg
                .ok_or_else(|| Error::Transfer(NEEDS_FFMPEG.to_owned()))?;
            let joined_audio = self.media_dir.join("audio.joined");
            join(audio, &joined_audio).await?;
            let out = self.media_dir.join("output.mp4");
            mux_to_mp4(program, &joined_video, &joined_audio, &out, cancel).await?;
            return Ok((out, "mp4"));
        }
        if video_is_mp4 {
            return Ok((joined_video, "mp4"));
        }
        let convert = self.ctx.settings.media.convert_to_mp4;
        match self.ffmpeg {
            Some(program) if convert => {
                let out = self.media_dir.join("output.mp4");
                match remux_to_mp4(program, &joined_video, &out, cancel).await {
                    Ok(()) => Ok((out, "mp4")),
                    Err(Error::Transfer(message)) if message == CANCELLED => {
                        Err(Error::Transfer(message))
                    }
                    Err(err) => {
                        let _ = std::fs::remove_file(&out);
                        tracing::warn!("cannot convert to mp4, keeping the TS file: {err}");
                        Ok((joined_video, "ts"))
                    }
                }
            }
            Some(_) => Ok((joined_video, "ts")),
            None => {
                if convert {
                    tracing::info!("ffmpeg not found, the stream stays an MPEG-TS file");
                }
                Ok((joined_video, "ts"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(bandwidth: u64) -> Variant {
        Variant {
            url: format!("v{bandwidth}.m3u8"),
            bandwidth,
            resolution: None,
            codecs: None,
            audio_group: None,
        }
    }

    #[test]
    fn quality_picks_highest_or_lowest_bandwidth() {
        let variants = [variant(800_000), variant(2_500_000), variant(400_000)];
        let best = choose_variant(&variants, MediaQuality::Best).expect("best");
        assert_eq!(best.bandwidth, 2_500_000);
        let worst = choose_variant(&variants, MediaQuality::Worst).expect("worst");
        assert_eq!(worst.bandwidth, 400_000);
        assert!(choose_variant(&[], MediaQuality::Best).is_none());
    }

    #[test]
    fn audio_prefers_the_default_rendition_of_the_group() {
        let renditions = [
            AudioRendition {
                group_id: "a".into(),
                url: Some("x.m3u8".into()),
                is_default: false,
            },
            AudioRendition {
                group_id: "a".into(),
                url: Some("y.m3u8".into()),
                is_default: true,
            },
            AudioRendition {
                group_id: "b".into(),
                url: Some("z.m3u8".into()),
                is_default: true,
            },
        ];
        assert_eq!(audio_url(&renditions, "a").as_deref(), Some("y.m3u8"));
        assert_eq!(audio_url(&renditions, "c"), None);
    }

    #[test]
    fn live_playlist_is_refused() {
        let media = MediaPlaylist::default();
        let message = checked(media).expect_err("live").to_string();
        assert_eq!(message, LIVE_MESSAGE);
    }
}
