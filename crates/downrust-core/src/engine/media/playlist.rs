// SPDX-License-Identifier: GPL-3.0-or-later
//! Parser for HLS master and media playlists. It covers the tags that common streams use:
//! variants, audio renditions, segments, byte ranges, AES-128 keys and the init map.
use std::collections::HashMap;

use url::Url;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Playlist {
    Master(MasterPlaylist),
    Media(MediaPlaylist),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MasterPlaylist {
    pub variants: Vec<Variant>,
    pub audio: Vec<AudioRendition>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Variant {
    pub url: String,
    pub bandwidth: u64,
    pub resolution: Option<(u32, u32)>,
    pub codecs: Option<String>,
    pub audio_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioRendition {
    pub group_id: String,
    pub url: Option<String>,
    pub is_default: bool,
}

/// A slice of a resource: `length` bytes starting at `offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteRange {
    pub length: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyInfo {
    pub url: String,
    pub iv: Option<[u8; 16]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Segment {
    pub url: String,
    pub duration: f64,
    pub byterange: Option<ByteRange>,
    pub key: Option<KeyInfo>,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapInfo {
    pub url: String,
    pub byterange: Option<ByteRange>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MediaPlaylist {
    pub segments: Vec<Segment>,
    pub map: Option<MapInfo>,
    pub ended: bool,
}

fn protocol(message: impl Into<String>) -> Error {
    Error::Protocol(message.into())
}

/// Parses a playlist. Relative addresses are resolved against `base`.
pub(crate) fn parse(text: &str, base: &Url) -> Result<Playlist> {
    if !text.trim_start().starts_with("#EXTM3U") {
        return Err(protocol("the address does not hold an HLS playlist"));
    }
    let mut master = MasterPlaylist::default();
    let mut media = MediaPlaylist::default();
    let mut is_master = false;
    let mut pending_variant: Option<Variant> = None;
    let mut pending_extinf = 0.0_f64;
    let mut pending_range: Option<(u64, Option<u64>)> = None;
    let mut key: Option<KeyInfo> = None;
    let mut media_sequence = 0_u64;
    let mut offsets: HashMap<String, u64> = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            is_master = true;
            pending_variant = Some(variant_from(rest)?);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA:") {
            is_master = true;
            if let Some(audio) = audio_from(rest, base)? {
                master.audio.push(audio);
            }
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            media_sequence = rest
                .trim()
                .parse()
                .map_err(|_| protocol("bad EXT-X-MEDIA-SEQUENCE"))?;
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            pending_extinf = rest
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0.0);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = Some(parse_range(rest)?);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-KEY:") {
            key = key_from(rest, base)?;
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            let attrs = parse_attributes(rest);
            let uri = attr(&attrs, "URI").ok_or_else(|| protocol("EXT-X-MAP without URI"))?;
            let url = resolve(base, uri)?;
            let byterange = match attr(&attrs, "BYTERANGE") {
                Some(value) => {
                    let (length, offset) = parse_range(value)?;
                    Some(resolve_range(&url, length, offset, &mut offsets))
                }
                None => None,
            };
            media.map = Some(MapInfo { url, byterange });
        } else if line == "#EXT-X-ENDLIST" {
            media.ended = true;
        } else if line.starts_with('#') {
            continue;
        } else {
            let url = resolve(base, line)?;
            if let Some(mut variant) = pending_variant.take() {
                variant.url = url;
                master.variants.push(variant);
            } else {
                let byterange = pending_range
                    .take()
                    .map(|(length, offset)| resolve_range(&url, length, offset, &mut offsets));
                let sequence = media_sequence + media.segments.len() as u64;
                media.segments.push(Segment {
                    url,
                    duration: pending_extinf,
                    byterange,
                    key: key.clone(),
                    sequence,
                });
                pending_extinf = 0.0;
            }
        }
    }
    if is_master {
        Ok(Playlist::Master(master))
    } else {
        Ok(Playlist::Media(media))
    }
}

/// Fills a missing offset with the end of the previous range of the same resource.
fn resolve_range(
    url: &str,
    length: u64,
    offset: Option<u64>,
    offsets: &mut HashMap<String, u64>,
) -> ByteRange {
    let start = offset.unwrap_or_else(|| offsets.get(url).copied().unwrap_or(0));
    offsets.insert(url.to_owned(), start + length);
    ByteRange {
        length,
        offset: start,
    }
}

fn resolve(base: &Url, value: &str) -> Result<String> {
    base.join(value.trim())
        .map(|url| url.to_string())
        .map_err(|err| protocol(format!("bad address in playlist: {err}")))
}

/// Parses `length[@offset]`.
fn parse_range(text: &str) -> Result<(u64, Option<u64>)> {
    let mut parts = text.trim().splitn(2, '@');
    let length = parts
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .map_err(|_| protocol("bad EXT-X-BYTERANGE length"))?;
    let offset = match parts.next() {
        Some(value) => Some(
            value
                .trim()
                .parse()
                .map_err(|_| protocol("bad EXT-X-BYTERANGE offset"))?,
        ),
        None => None,
    };
    Ok((length, offset))
}

/// Splits an attribute list such as `A=1,B="x,y"`. Quoted values may contain commas.
pub(crate) fn parse_attributes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else {
            break;
        };
        let name = rest[..eq].trim().to_ascii_uppercase();
        let after = &rest[eq + 1..];
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => match quoted.find('"') {
                Some(end) => (&quoted[..end], &quoted[end + 1..]),
                None => (quoted, ""),
            },
            None => match after.find(',') {
                Some(end) => (&after[..end], &after[end..]),
                None => (after, ""),
            },
        };
        out.push((name, value.to_owned()));
        rest = next.trim_start_matches(',').trim_start();
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn variant_from(rest: &str) -> Result<Variant> {
    let attrs = parse_attributes(rest);
    let bandwidth = attr(&attrs, "BANDWIDTH")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| protocol("EXT-X-STREAM-INF without BANDWIDTH"))?;
    let resolution = attr(&attrs, "RESOLUTION").and_then(|value| {
        let (w, h) = value.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    });
    Ok(Variant {
        url: String::new(),
        bandwidth,
        resolution,
        codecs: attr(&attrs, "CODECS").map(str::to_owned),
        audio_group: attr(&attrs, "AUDIO").map(str::to_owned),
    })
}

/// Returns the rendition when it is an audio track with a group id.
fn audio_from(rest: &str, base: &Url) -> Result<Option<AudioRendition>> {
    let attrs = parse_attributes(rest);
    if attr(&attrs, "TYPE") != Some("AUDIO") {
        return Ok(None);
    }
    let Some(group_id) = attr(&attrs, "GROUP-ID") else {
        return Ok(None);
    };
    let url = match attr(&attrs, "URI") {
        Some(value) => Some(resolve(base, value)?),
        None => None,
    };
    Ok(Some(AudioRendition {
        group_id: group_id.to_owned(),
        url,
        is_default: attr(&attrs, "DEFAULT") == Some("YES"),
    }))
}

/// `None` for METHOD=NONE, which removes the key for the following segments.
fn key_from(rest: &str, base: &Url) -> Result<Option<KeyInfo>> {
    let attrs = parse_attributes(rest);
    match attr(&attrs, "METHOD") {
        Some("NONE") => Ok(None),
        Some("AES-128") => {
            let uri = attr(&attrs, "URI").ok_or_else(|| protocol("AES-128 key without URI"))?;
            let iv = match attr(&attrs, "IV") {
                Some(value) => Some(parse_iv(value)?),
                None => None,
            };
            Ok(Some(KeyInfo {
                url: resolve(base, uri)?,
                iv,
            }))
        }
        Some(other) => Err(protocol(format!("unsupported encryption method {other}"))),
        None => Err(protocol("EXT-X-KEY without METHOD")),
    }
}

/// Parses a 16-byte initialization vector written as hexadecimal with a `0x` prefix.
pub(crate) fn parse_iv(text: &str) -> Result<[u8; 16]> {
    let trimmed = text.trim();
    let hex_text = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .ok_or_else(|| protocol("IV must start with 0x"))?;
    let bytes = hex::decode(hex_text).map_err(|_| protocol("IV is not hexadecimal"))?;
    <[u8; 16]>::try_from(bytes.as_slice()).map_err(|_| protocol("IV must be 16 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://cdn.example.test/show/index.m3u8?sig=1").expect("base url")
    }

    fn media(text: &str) -> MediaPlaylist {
        match parse(text, &base()).expect("parses") {
            Playlist::Media(media) => media,
            Playlist::Master(_) => panic!("expected a media playlist"),
        }
    }

    #[test]
    fn master_lists_variants_and_audio() {
        let text = concat!(
            "#EXTM3U\n",
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English, main\",URI=\"audio/en.m3u8\",DEFAULT=YES\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS=\"avc1.4d401e,mp4a.40.2\",AUDIO=\"aud\"\n",
            "360/index.m3u8\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720\n",
            "720/index.m3u8\n",
        );
        let Playlist::Master(master) = parse(text, &base()).expect("parses") else {
            panic!("expected master");
        };
        assert_eq!(master.variants.len(), 2);
        assert_eq!(master.variants[0].bandwidth, 800_000);
        assert_eq!(master.variants[0].resolution, Some((640, 360)));
        assert_eq!(
            master.variants[0].codecs.as_deref(),
            Some("avc1.4d401e,mp4a.40.2")
        );
        assert_eq!(master.variants[0].audio_group.as_deref(), Some("aud"));
        assert_eq!(
            master.variants[0].url,
            "https://cdn.example.test/show/360/index.m3u8"
        );
        assert_eq!(master.variants[1].audio_group, None);
        assert_eq!(master.audio.len(), 1);
        assert_eq!(master.audio[0].group_id, "aud");
        assert!(master.audio[0].is_default);
        assert_eq!(
            master.audio[0].url.as_deref(),
            Some("https://cdn.example.test/show/audio/en.m3u8")
        );
    }

    #[test]
    fn attributes_keep_quoted_commas() {
        let attrs = parse_attributes("A=\"x,y\",B=2,C=\"\",D=last");
        assert_eq!(attr(&attrs, "A"), Some("x,y"));
        assert_eq!(attr(&attrs, "B"), Some("2"));
        assert_eq!(attr(&attrs, "C"), Some(""));
        assert_eq!(attr(&attrs, "D"), Some("last"));
    }

    #[test]
    fn segments_get_durations_sequence_and_absolute_urls() {
        let text = concat!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:7\n",
            "#EXTINF:3.5,\nseg7.ts\n#EXTINF:4.0,\n/abs/seg8.ts\n#EXT-X-ENDLIST\n",
        );
        let media = media(text);
        assert!(media.ended);
        assert_eq!(media.segments.len(), 2);
        assert_eq!(
            media.segments[0].url,
            "https://cdn.example.test/show/seg7.ts"
        );
        assert_eq!(media.segments[0].duration, 3.5);
        assert_eq!(media.segments[0].sequence, 7);
        assert_eq!(
            media.segments[1].url,
            "https://cdn.example.test/abs/seg8.ts"
        );
        assert_eq!(media.segments[1].sequence, 8);
    }

    #[test]
    fn live_playlist_has_no_end_marker() {
        let media = media("#EXTM3U\n#EXTINF:2.0,\na.ts\n");
        assert!(!media.ended);
    }

    #[test]
    fn byte_ranges_continue_from_the_previous_part() {
        let text = concat!(
            "#EXTM3U\n",
            "#EXTINF:2,\n#EXT-X-BYTERANGE:100@0\nbig.ts\n",
            "#EXTINF:2,\n#EXT-X-BYTERANGE:50\nbig.ts\n",
            "#EXTINF:2,\n#EXT-X-BYTERANGE:10@500\nbig.ts\n",
            "#EXTINF:2,\n#EXT-X-BYTERANGE:5\nother.ts\n",
        );
        let media = media(text);
        let ranges: Vec<ByteRange> = media
            .segments
            .iter()
            .map(|s| s.byterange.expect("range"))
            .collect();
        assert_eq!(
            ranges[0],
            ByteRange {
                length: 100,
                offset: 0
            }
        );
        assert_eq!(
            ranges[1],
            ByteRange {
                length: 50,
                offset: 100
            }
        );
        assert_eq!(
            ranges[2],
            ByteRange {
                length: 10,
                offset: 500
            }
        );
        assert_eq!(
            ranges[3],
            ByteRange {
                length: 5,
                offset: 0
            }
        );
    }

    #[test]
    fn key_and_map_are_read() {
        let text = concat!(
            "#EXTM3U\n",
            "#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.test/k?a=1,2\",",
            "IV=0x000102030405060708090a0b0c0d0e0f\n",
            "#EXTINF:2,\nc1.m4s\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:2,\nc2.m4s\n",
        );
        let media = media(text);
        let map = media.map.clone().expect("map");
        assert_eq!(map.url, "https://cdn.example.test/show/init.mp4");
        assert_eq!(
            map.byterange,
            Some(ByteRange {
                length: 720,
                offset: 0
            })
        );
        let key = media.segments[0].key.clone().expect("key");
        assert_eq!(key.url, "https://keys.example.test/k?a=1,2");
        assert_eq!(key.iv.expect("iv")[15], 0x0f);
        assert_eq!(media.segments[1].key, None);
    }

    #[test]
    fn unsupported_or_broken_input_is_an_error() {
        assert!(parse("<html></html>", &base()).is_err());
        let sample_aes = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"\n";
        assert!(parse(sample_aes, &base()).is_err());
        let no_bandwidth = "#EXTM3U\n#EXT-X-STREAM-INF:RESOLUTION=1x1\nv.m3u8\n";
        assert!(parse(no_bandwidth, &base()).is_err());
        assert!(parse_iv("0x0102").is_err());
        assert!(parse_iv("00112233445566778899aabbccddeeff").is_err());
    }
}
