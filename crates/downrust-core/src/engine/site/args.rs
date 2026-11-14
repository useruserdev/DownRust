// SPDX-License-Identifier: GPL-3.0-or-later
//! Command line of yt-dlp: request headers, proxy, quality and the output folder.
use std::path::Path;

use crate::model::HeaderPair;
use crate::secrets;
use crate::settings::{route_for, EngineSettings, Route, SiteFormat};

/// Progress line printed for every update. Values can be `NA`.
/// Format order for merged files: the highest resolution first, and at the same resolution
/// H.264 video and AAC audio, which every player opens inside an MP4 file.
const COMPATIBLE_SORT: &str = "res,fps,vcodec:h264,acodec:m4a";

pub(crate) const PROGRESS_TEMPLATE: &str = "download:DRP %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.speed)s";

/// The proxy address for a page, when the proxy rules send it through the proxy. The proxy
/// password is added from the credential store.
pub(crate) fn proxy_for(settings: &EngineSettings, url: &str) -> Option<String> {
    let host = url::Url::parse(url.trim()).ok()?.host_str()?.to_owned();
    if route_for(&settings.proxy, &host) != Route::Proxy {
        return None;
    }
    let mut address = url::Url::parse(&settings.proxy.url).ok()?;
    let user = settings.proxy.username.as_str();
    if !user.is_empty() {
        let _ = address.set_username(user);
        let password = secrets::get(secrets::PROXY_USER).ok().flatten();
        let _ = address.set_password(password.as_deref());
    }
    Some(address.to_string())
}

/// Options for the page request: the user agent, the referrer, the proxy and every other header.
pub(crate) fn request_args(
    user_agent: &str,
    headers: &[HeaderPair],
    proxy: Option<String>,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(proxy) = proxy {
        args.extend(["--proxy".to_owned(), proxy]);
    }
    let mut agent = user_agent.to_owned();
    let mut referer = None;
    let mut others = Vec::new();
    for header in headers {
        if header.name.eq_ignore_ascii_case("user-agent") {
            agent = header.value.clone();
        } else if header.name.eq_ignore_ascii_case("referer") {
            referer = Some(header.value.clone());
        } else {
            others.push(format!("{}:{}", header.name, header.value));
        }
    }
    args.extend(["--user-agent".to_owned(), agent]);
    if let Some(referer) = referer {
        args.extend(["--referer".to_owned(), referer]);
    }
    for other in others {
        args.extend(["--add-header".to_owned(), other]);
    }
    args
}

/// The yt-dlp format selector. With ffmpeg, video and audio are merged. Without it, one file
/// with both is chosen.
pub(crate) fn format_selector(format: SiteFormat, has_ffmpeg: bool) -> String {
    match (format.is_audio(), format.max_height(), has_ffmpeg) {
        (true, _, true) => "ba/b".to_owned(),
        (true, _, false) => "ba".to_owned(),
        (false, None, true) => "bv*+ba/b".to_owned(),
        (false, None, false) => "b".to_owned(),
        (false, Some(height), true) => {
            format!("bv*[height<={height}]+ba/b[height<={height}]")
        }
        (false, Some(height), false) => format!("b[height<={height}]"),
    }
}

/// Full command line of a download. Progress goes to stdout, one line per update.
pub(crate) fn download_args(
    url: &str,
    work_dir: &Path,
    format: SiteFormat,
    connections: u8,
    rate: Option<u64>,
    ffmpeg: Option<&Path>,
    request: &[String],
) -> Vec<String> {
    let output = work_dir.join("video.%(ext)s");
    let mut args: Vec<String> = [
        "--newline",
        "--no-playlist",
        "--no-mtime",
        "--continue",
        "--no-colors",
        "-N",
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    args.push(connections.clamp(1, 8).to_string());
    args.extend([
        "-o".to_owned(),
        output.to_string_lossy().into_owned(),
        "--progress-template".to_owned(),
        PROGRESS_TEMPLATE.to_owned(),
        "-f".to_owned(),
        format_selector(format, ffmpeg.is_some()),
    ]);
    if let Some(rate) = rate {
        args.extend(["-r".to_owned(), rate.to_string()]);
    }
    if let Some(ffmpeg) = ffmpeg {
        args.extend([
            "--ffmpeg-location".to_owned(),
            ffmpeg.to_string_lossy().into_owned(),
            "--merge-output-format".to_owned(),
            "mp4".to_owned(),
            "-S".to_owned(),
            COMPATIBLE_SORT.to_owned(),
        ]);
    }
    args.extend(request.iter().cloned());
    args.push(url.to_owned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(name: &str, value: &str) -> HeaderPair {
        HeaderPair {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn selector_depends_on_ffmpeg() {
        assert_eq!(format_selector(SiteFormat::Best, true), "bv*+ba/b");
        assert_eq!(format_selector(SiteFormat::Best, false), "b");
        assert_eq!(
            format_selector(SiteFormat::P1080, true),
            "bv*[height<=1080]+ba/b[height<=1080]"
        );
        assert_eq!(format_selector(SiteFormat::P720, false), "b[height<=720]");
        assert_eq!(format_selector(SiteFormat::AudioOnly, true), "ba/b");
        assert_eq!(format_selector(SiteFormat::AudioOnly, false), "ba");
    }

    #[test]
    fn request_headers_map_to_options() {
        let headers = [
            header("Cookie", "a=1"),
            header("User-Agent", "Browser/1"),
            header("Referer", "https://page.test/"),
        ];
        let args = request_args("fallback", &headers, Some("http://proxy:8080".to_owned()));
        assert_eq!(
            args,
            vec![
                "--proxy",
                "http://proxy:8080",
                "--user-agent",
                "Browser/1",
                "--referer",
                "https://page.test/",
                "--add-header",
                "Cookie:a=1",
            ]
        );
        assert_eq!(
            request_args("fallback", &[], None),
            vec!["--user-agent", "fallback"]
        );
    }

    #[test]
    fn download_command_has_rate_ffmpeg_and_url() {
        let request = request_args("ua", &[], None);
        let args = download_args(
            "https://youtu.be/x",
            Path::new("C:/work"),
            SiteFormat::P1080,
            12,
            Some(500_000),
            Some(Path::new("C:/bin/ffmpeg.exe")),
            &request,
        );
        assert!(args.windows(2).any(|pair| pair == ["-N", "8"]));
        assert!(args.windows(2).any(|pair| pair == ["-r", "500000"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--merge-output-format", "mp4"]));
        assert!(args.windows(2).any(|pair| pair == ["-S", COMPATIBLE_SORT]));
        assert!(args.iter().any(|arg| arg == PROGRESS_TEMPLATE));
        assert_eq!(args.last().map(String::as_str), Some("https://youtu.be/x"));
    }

    #[test]
    fn download_command_without_limits_or_ffmpeg() {
        let args = download_args(
            "https://youtu.be/x",
            Path::new("C:/work"),
            SiteFormat::Best,
            1,
            None,
            None,
            &[],
        );
        assert!(!args.iter().any(|arg| arg == "-r"));
        assert!(!args.iter().any(|arg| arg == "--ffmpeg-location"));
        assert!(args.windows(2).any(|pair| pair == ["-f", "b"]));
    }
}
