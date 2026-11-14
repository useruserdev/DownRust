// SPDX-License-Identifier: GPL-3.0-or-later
//! Recognizes page addresses of video sites that yt-dlp can download.
use url::Url;

/// One site. `paths` lists parts a page address must contain. An empty list means any path.
struct Rule {
    host: &'static str,
    paths: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        host: "youtube.com",
        paths: &[],
    },
    Rule {
        host: "youtu.be",
        paths: &[],
    },
    Rule {
        host: "youtube-nocookie.com",
        paths: &[],
    },
    Rule {
        host: "vimeo.com",
        paths: &[],
    },
    Rule {
        host: "dailymotion.com",
        paths: &[],
    },
    Rule {
        host: "twitch.tv",
        paths: &[],
    },
    Rule {
        host: "tiktok.com",
        paths: &[],
    },
    Rule {
        host: "vk.com",
        paths: &["video", "clip"],
    },
    Rule {
        host: "vkvideo.ru",
        paths: &[],
    },
    Rule {
        host: "rutube.ru",
        paths: &[],
    },
    Rule {
        host: "ok.ru",
        paths: &["/video"],
    },
    Rule {
        host: "dzen.ru",
        paths: &["/video"],
    },
    Rule {
        host: "x.com",
        paths: &["/status/"],
    },
    Rule {
        host: "twitter.com",
        paths: &["/status/"],
    },
    Rule {
        host: "instagram.com",
        paths: &["/p/", "/reel"],
    },
    Rule {
        host: "facebook.com",
        paths: &["/watch", "/videos/"],
    },
    Rule {
        host: "fb.watch",
        paths: &[],
    },
    Rule {
        host: "reddit.com",
        paths: &["/comments/"],
    },
    Rule {
        host: "bilibili.com",
        paths: &[],
    },
    Rule {
        host: "soundcloud.com",
        paths: &[],
    },
    Rule {
        host: "bandcamp.com",
        paths: &[],
    },
    Rule {
        host: "coub.com",
        paths: &[],
    },
    Rule {
        host: "streamable.com",
        paths: &[],
    },
];

/// True when the address is an http or https page of a listed video site. Subdomains match.
pub fn is_video_site(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    RULES.iter().any(|rule| {
        host_matches(&host, rule.host)
            && (rule.paths.is_empty() || rule.paths.iter().any(|part| path.contains(part)))
    })
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_video_hosts_match_with_subdomains() {
        assert!(is_video_site("https://www.youtube.com/watch?v=abc"));
        assert!(is_video_site("https://youtu.be/abc"));
        assert!(is_video_site("https://m.vimeo.com/12345"));
        assert!(is_video_site("https://rutube.ru/video/1/"));
    }

    #[test]
    fn some_sites_need_a_video_path() {
        assert!(is_video_site("https://vk.com/video-1_2"));
        assert!(!is_video_site("https://vk.com/friends"));
        assert!(is_video_site("https://x.com/user/status/1"));
        assert!(!is_video_site("https://x.com/user"));
        assert!(is_video_site("https://www.instagram.com/reel/abc/"));
        assert!(!is_video_site("https://www.instagram.com/someone/"));
        assert!(is_video_site("https://www.facebook.com/watch/?v=1"));
        assert!(!is_video_site("https://www.facebook.com/someone"));
        assert!(is_video_site("https://ok.ru/video/12"));
        assert!(!is_video_site("https://ok.ru/group/1"));
        assert!(is_video_site("https://www.reddit.com/r/x/comments/abc/t/"));
    }

    #[test]
    fn other_addresses_do_not_match() {
        assert!(!is_video_site("https://notyoutube.com/watch"));
        assert!(!is_video_site("https://example.com/video.mp4"));
        assert!(!is_video_site("ftp://youtube.com/x"));
        assert!(!is_video_site("youtube.com/watch?v=abc"));
        assert!(!is_video_site("magnet:?xt=urn:btih:abc"));
    }
}
