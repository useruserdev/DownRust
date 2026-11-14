// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads the details of a video page with `yt-dlp -J`.
use std::process::Stdio;

use serde::{Deserialize, Serialize};

use super::args::{proxy_for, request_args};
use super::ensure_tool;
use super::error_message;
use crate::engine::media::ffmpeg::hidden_std_command;
use crate::error::{Error, Result};
use crate::model::HeaderPair;
use crate::settings::EngineSettings;
use crate::util::filename::sanitize_file_name;

/// What the add dialog shows for a video page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteInfo {
    pub title: String,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    /// Name of the extractor, for example `Youtube`.
    pub site: String,
    /// Distinct video heights, highest first.
    pub heights: Vec<u32>,
    pub has_audio: bool,
    /// Sanitized title with the `.mp4` extension.
    pub file_name: String,
}

#[derive(Deserialize)]
struct RawInfo {
    title: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    extractor_key: Option<String>,
    formats: Option<Vec<RawFormat>>,
}

#[derive(Deserialize)]
struct RawFormat {
    height: Option<f64>,
    vcodec: Option<String>,
    acodec: Option<String>,
}

/// Reads the page at `url` with yt-dlp. The headers and the proxy are used as for a download.
pub async fn site_info(
    settings: &EngineSettings,
    client: &reqwest::Client,
    url: &str,
    headers: &[HeaderPair],
) -> Result<SiteInfo> {
    let tool = ensure_tool(settings, client).await?;
    let mut args = request_args(&settings.user_agent, headers, proxy_for(settings, url));
    args.extend(
        ["-J", "--no-playlist", "--no-warnings"]
            .iter()
            .map(|arg| (*arg).to_owned()),
    );
    args.push(url.to_owned());
    let output = tokio::process::Command::from(hidden_std_command(&tool))
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Transfer(error_message(&stderr)));
    }
    parse_info(&String::from_utf8_lossy(&output.stdout))
}

/// Reads the JSON that `yt-dlp -J` prints. Missing fields get simple defaults.
pub(crate) fn parse_info(json: &str) -> Result<SiteInfo> {
    let raw: RawInfo = serde_json::from_str(json)?;
    let title = raw.title.unwrap_or_default();
    let base = if title.trim().is_empty() {
        "video".to_owned()
    } else {
        title.clone()
    };
    let formats = raw.formats.unwrap_or_default();
    let mut heights: Vec<u32> = formats
        .iter()
        .filter(|format| has_codec(format.vcodec.as_deref()))
        .filter_map(|format| format.height)
        .filter(|height| *height > 0.0)
        .map(|height| height as u32)
        .collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    let has_audio = formats
        .iter()
        .any(|format| has_codec(format.acodec.as_deref()));
    Ok(SiteInfo {
        title,
        duration_secs: raw.duration,
        thumbnail: raw.thumbnail,
        site: raw.extractor_key.unwrap_or_default(),
        heights,
        has_audio,
        file_name: format!("{}.mp4", sanitize_file_name(&base)),
    })
}

/// True when a codec field names a real codec. yt-dlp writes `none` for a missing one.
fn has_codec(codec: Option<&str>) -> bool {
    codec.is_some_and(|codec| codec != "none")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "id": "abc",
        "title": "Trip: Day 1/2",
        "duration": 615.5,
        "thumbnail": "https://img.test/a.jpg",
        "extractor_key": "Youtube",
        "formats": [
            {"format_id": "18", "height": 360, "vcodec": "avc1", "acodec": "mp4a"},
            {"format_id": "137", "height": 1080, "vcodec": "avc1", "acodec": "none"},
            {"format_id": "136", "height": 720, "vcodec": "avc1", "acodec": "none"},
            {"format_id": "140", "height": null, "vcodec": "none", "acodec": "mp4a"},
            {"format_id": "137b", "height": 1080.0, "vcodec": "vp9", "acodec": "none"}
        ]
    }"#;

    #[test]
    fn info_is_read_from_the_json() {
        let info = parse_info(FIXTURE).expect("parses");
        assert_eq!(info.title, "Trip: Day 1/2");
        assert_eq!(info.duration_secs, Some(615.5));
        assert_eq!(info.thumbnail.as_deref(), Some("https://img.test/a.jpg"));
        assert_eq!(info.site, "Youtube");
        assert_eq!(info.heights, vec![1080, 720, 360]);
        assert!(info.has_audio);
        assert_eq!(info.file_name, "Trip_ Day 1_2.mp4");
    }

    #[test]
    fn missing_fields_get_defaults() {
        let info = parse_info("{}").expect("parses");
        assert_eq!(info.title, "");
        assert_eq!(info.file_name, "video.mp4");
        assert!(info.heights.is_empty());
        assert!(!info.has_audio);
        assert_eq!(info.site, "");
        let nulls = parse_info(r#"{"title": null, "formats": null}"#).expect("parses");
        assert_eq!(nulls.file_name, "video.mp4");
    }

    #[test]
    fn broken_json_is_an_error() {
        assert!(parse_info("not json").is_err());
    }
}
