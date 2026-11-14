// SPDX-License-Identifier: GPL-3.0-or-later
//! HLS fixtures. `/hls/master.m3u8` lists a `low` and a `high` variant. `/hls/enc/` holds
//! AES-128 segments with the key at `/hls/key`. `/hls/live/` has no end marker. `/hls/br/` cuts
//! one file into byte ranges. Segment bytes are a fixed formula, so tests can compute the output.
use std::sync::Arc;

use aes::Aes128;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};

use crate::state::ServerState;

/// Parts in each variant and in the encrypted list.
pub const HLS_SEGMENTS: usize = 12;
pub const HLS_ENC_SEGMENTS: usize = 6;
/// Bytes in one part of the byte-range fixture. The file has four parts.
pub const HLS_BYTE_RANGE_BYTES: usize = 4096;
const BYTE_RANGE_PARTS: usize = 4;
/// Plain bytes of each encrypted part. Not a multiple of 16, so padding is used.
const ENC_PLAIN_BYTES: usize = 10_000;
/// AES-128 key of the encrypted fixture.
pub const HLS_KEY: [u8; 16] = *b"downrust-test-k1";

type Aes128CbcEncryptor = cbc::Encryptor<Aes128>;

/// Bytes of one part of a variant. `high` parts are 64 KiB, `low` parts are 16 KiB.
fn part_len(variant: &str) -> usize {
    if variant == "high" {
        64 * 1024
    } else {
        16 * 1024
    }
}

/// Deterministic bytes of one part.
fn part_bytes(name: &str, index: usize, len: usize) -> Vec<u8> {
    let seed = name.bytes().fold(0_usize, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(usize::from(byte))
    });
    (0..len)
        .map(|i| ((index * 131 + i * 7 + seed) % 251) as u8)
        .collect()
}

/// Plain bytes of one encrypted part.
pub fn enc_segment_plain(index: usize) -> Vec<u8> {
    part_bytes("enc", index, ENC_PLAIN_BYTES)
}

/// The bytes a finished download of a fixture must have. `variant` is `low`, `high`, `enc`
/// (decrypted) or `br` (the byte-range file).
pub fn hls_variant_bytes(variant: &str) -> Vec<u8> {
    match variant {
        "enc" => (0..HLS_ENC_SEGMENTS).flat_map(enc_segment_plain).collect(),
        "br" => (0..BYTE_RANGE_PARTS)
            .flat_map(|index| part_bytes("br", index, HLS_BYTE_RANGE_BYTES))
            .collect(),
        other => (0..HLS_SEGMENTS)
            .flat_map(|index| part_bytes(other, index, part_len(other)))
            .collect(),
    }
}

const MASTER: &str = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=400000,RESOLUTION=320x180\n\
low/index.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720\n\
high/index.m3u8\n";

const LIVE: &str = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\n/hls/high/seg0.ts\n\
#EXTINF:4.0,\n/hls/high/seg1.ts\n";

/// Media playlist with `count` parts, each 4 seconds long, named `seg{n}.ts`.
fn media_index(count: usize, key_line: &str) -> String {
    let mut text = format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:0\n{key_line}"
    );
    for index in 0..count {
        text.push_str(&format!("#EXTINF:4.0,\nseg{index}.ts\n"));
    }
    text.push_str("#EXT-X-ENDLIST\n");
    text
}

fn byte_range_index() -> String {
    let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
    for index in 0..BYTE_RANGE_PARTS {
        text.push_str("#EXTINF:4.0,\n");
        if index == 0 {
            text.push_str(&format!("#EXT-X-BYTERANGE:{HLS_BYTE_RANGE_BYTES}@0\n"));
        } else {
            text.push_str(&format!("#EXT-X-BYTERANGE:{HLS_BYTE_RANGE_BYTES}\n"));
        }
        text.push_str("all.ts\n");
    }
    text.push_str("#EXT-X-ENDLIST\n");
    text
}

fn parse_part(file: &str) -> Option<usize> {
    file.strip_prefix("seg")?.strip_suffix(".ts")?.parse().ok()
}

/// AES-128 CBC with PKCS#7 padding. The IV is the part number as 16-byte big-endian.
fn encrypt(index: usize) -> Vec<u8> {
    let mut iv = [0_u8; 16];
    iv[8..].copy_from_slice(&(index as u64).to_be_bytes());
    Aes128CbcEncryptor::new((&HLS_KEY).into(), (&iv).into())
        .encrypt_padded_vec_mut::<Pkcs7>(&enc_segment_plain(index))
}

fn text(body: &str, content_type: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body.to_owned()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn binary(body: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Answers a `bytes=a-b` range with 206, or the whole body without a range.
fn ranged(bytes: Vec<u8>, headers: &HeaderMap) -> Response {
    let Some(spec) = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes="))
    else {
        return binary(bytes);
    };
    let Some((start, end)) = spec.split_once('-') else {
        return not_found();
    };
    let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) else {
        return not_found();
    };
    let Some(slice) = bytes.get(start..=end.min(bytes.len().saturating_sub(1))) else {
        return not_found();
    };
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{}", start + slice.len() - 1, bytes.len()),
        )
        .body(Body::from(slice.to_vec()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Serves one HLS fixture path.
pub(crate) async fn hls(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.count_request();
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["master.m3u8"] => text(MASTER, "application/vnd.apple.mpegurl"),
        ["key"] => binary(HLS_KEY.to_vec()),
        ["live", "index.m3u8"] => text(LIVE, "application/vnd.apple.mpegurl"),
        ["enc", "index.m3u8"] => text(
            &media_index(
                HLS_ENC_SEGMENTS,
                "#EXT-X-KEY:METHOD=AES-128,URI=\"/hls/key\"\n",
            ),
            "application/vnd.apple.mpegurl",
        ),
        ["br", "index.m3u8"] => text(&byte_range_index(), "application/vnd.apple.mpegurl"),
        ["br", "all.ts"] => ranged(hls_variant_bytes("br"), &headers),
        ["enc", file] => match parse_part(file) {
            Some(index) if index < HLS_ENC_SEGMENTS => {
                let delay = state.count_hls_segment();
                pause(delay).await;
                binary(encrypt(index))
            }
            _ => not_found(),
        },
        ["low" | "high", "index.m3u8"] => text(
            &media_index(HLS_SEGMENTS, ""),
            "application/vnd.apple.mpegurl",
        ),
        [variant @ ("low" | "high"), file] => match parse_part(file) {
            Some(index) if index < HLS_SEGMENTS => {
                let delay = state.count_hls_segment();
                pause(delay).await;
                binary(part_bytes(variant, index, part_len(variant)))
            }
            _ => not_found(),
        },
        _ => not_found(),
    }
}

async fn pause(ms: u64) {
    if ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockDecryptMut;

    #[test]
    fn fixture_sizes_match_the_plan() {
        assert_eq!(hls_variant_bytes("high").len(), HLS_SEGMENTS * 64 * 1024);
        assert_eq!(hls_variant_bytes("low").len(), HLS_SEGMENTS * 16 * 1024);
        assert_eq!(
            hls_variant_bytes("br").len(),
            BYTE_RANGE_PARTS * HLS_BYTE_RANGE_BYTES
        );
    }

    #[test]
    fn encrypted_part_decrypts_to_the_plain_bytes() {
        let cipher = encrypt(2);
        let mut iv = [0_u8; 16];
        iv[8..].copy_from_slice(&2_u64.to_be_bytes());
        let plain = cbc::Decryptor::<Aes128>::new((&HLS_KEY).into(), (&iv).into())
            .decrypt_padded_vec_mut::<Pkcs7>(&cipher)
            .expect("decrypts");
        assert_eq!(plain, enc_segment_plain(2));
    }
}
