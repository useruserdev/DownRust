// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic test content. The same version and size always produce the same bytes.

const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// Seed for a file of the given version and size.
fn seed(version: u64, size: u64) -> u64 {
    version.wrapping_mul(GAMMA) ^ size
}

/// Splitmix64 finalizer.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The 8 bytes of block `index` (little-endian).
fn block(seed: u64, index: u64) -> [u8; 8] {
    let state = seed.wrapping_add(index.wrapping_add(1).wrapping_mul(GAMMA));
    mix(state).to_le_bytes()
}

/// Fills `buf` with the bytes of a file at absolute position `offset`.
pub fn fill(version: u64, size: u64, offset: u64, buf: &mut [u8]) {
    let seed = seed(version, size);
    let mut pos = offset;
    let mut done = 0usize;
    while done < buf.len() {
        let bytes = block(seed, pos / 8);
        let within = (pos % 8) as usize;
        let n = (8 - within).min(buf.len() - done);
        buf[done..done + n].copy_from_slice(&bytes[within..within + n]);
        done += n;
        pos += n as u64;
    }
}

/// The whole content of a file of the given version and size.
pub fn full_content(version: u64, size: u64) -> Vec<u8> {
    let mut out = vec![0u8; size as usize];
    fill(version, size, 0, &mut out);
    out
}

/// Strong ETag value, quotes included.
pub fn etag(version: u64, size: u64) -> String {
    format!("\"v{version}-{size}\"")
}

/// Last-Modified value. The minute part follows the version.
pub fn last_modified(version: u64) -> String {
    format!("Wed, 01 May 2024 12:{:02}:00 GMT", version % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_deterministic() {
        assert_eq!(full_content(1, 100), full_content(1, 100));
    }

    #[test]
    fn version_and_size_change_content() {
        assert_ne!(full_content(1, 100), full_content(2, 100));
        assert_ne!(full_content(1, 100)[..50], full_content(1, 101)[..50]);
    }

    #[test]
    fn partial_fill_matches_full_content() {
        let full = full_content(3, 1000);
        for (offset, len) in [(0usize, 10usize), (5, 17), (991, 9), (8, 8), (7, 2)] {
            let mut buf = vec![0u8; len];
            fill(3, 1000, offset as u64, &mut buf);
            assert_eq!(buf, full[offset..offset + len]);
        }
    }

    #[test]
    fn etag_and_last_modified_format() {
        assert_eq!(etag(2, 1000), "\"v2-1000\"");
        assert_eq!(last_modified(61), "Wed, 01 May 2024 12:01:00 GMT");
    }
}
