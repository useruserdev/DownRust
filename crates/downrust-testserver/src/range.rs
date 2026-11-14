// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing of the `Range` and `If-Range` request headers.

/// What the response for a file request must look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Requested {
    /// Send the whole file with `200`.
    Full,
    /// Send bytes `start..=end` with `206`.
    Partial { start: u64, end: u64 },
    /// The range lies outside the file. Send `416`.
    Unsatisfiable,
}

/// Decides the response for a `Range` header value against a file of `size` bytes.
///
/// Accepts `a-b`, `a-` and `-n`. A multi-range request, or a header that is
/// not understood, is served in full with `200`.
pub(crate) fn evaluate(header: Option<&str>, size: u64) -> Requested {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Requested::Full;
    };
    if spec.contains(',') {
        return Requested::Full;
    }
    let Some((first, last)) = spec.split_once('-') else {
        return Requested::Full;
    };
    let (first, last) = (first.trim(), last.trim());
    if first.is_empty() {
        let Ok(suffix) = last.parse::<u64>() else {
            return Requested::Full;
        };
        if suffix == 0 || size == 0 {
            return Requested::Unsatisfiable;
        }
        return Requested::Partial {
            start: size.saturating_sub(suffix),
            end: size - 1,
        };
    }
    let Ok(start) = first.parse::<u64>() else {
        return Requested::Full;
    };
    if start >= size {
        return Requested::Unsatisfiable;
    }
    let end = if last.is_empty() {
        size - 1
    } else {
        match last.parse::<u64>() {
            Ok(end) if end >= start => end.min(size - 1),
            _ => return Requested::Full,
        }
    };
    Requested::Partial { start, end }
}

/// True when an `If-Range` value matches the current ETag or Last-Modified value.
pub(crate) fn if_range_matches(value: &str, etag: Option<&str>, last_modified: &str) -> bool {
    let value = value.trim();
    etag == Some(value) || value == last_modified
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u64 = 1000;

    #[test]
    fn explicit_range_is_partial() {
        assert_eq!(
            evaluate(Some("bytes=0-9"), SIZE),
            Requested::Partial { start: 0, end: 9 }
        );
    }

    #[test]
    fn open_and_suffix_ranges() {
        assert_eq!(
            evaluate(Some("bytes=990-"), SIZE),
            Requested::Partial {
                start: 990,
                end: 999
            }
        );
        assert_eq!(
            evaluate(Some("bytes=-10"), SIZE),
            Requested::Partial {
                start: 990,
                end: 999
            }
        );
        assert_eq!(
            evaluate(Some("bytes=-5000"), SIZE),
            Requested::Partial { start: 0, end: 999 }
        );
    }

    #[test]
    fn end_past_file_is_clamped() {
        assert_eq!(
            evaluate(Some("bytes=0-5000"), SIZE),
            Requested::Partial { start: 0, end: 999 }
        );
    }

    #[test]
    fn unsatisfiable_ranges() {
        assert_eq!(
            evaluate(Some("bytes=1000-"), SIZE),
            Requested::Unsatisfiable
        );
        assert_eq!(evaluate(Some("bytes=-0"), SIZE), Requested::Unsatisfiable);
        assert_eq!(evaluate(Some("bytes=0-0"), 0), Requested::Unsatisfiable);
    }

    #[test]
    fn ignored_headers_are_full() {
        assert_eq!(evaluate(None, SIZE), Requested::Full);
        assert_eq!(evaluate(Some("bytes=0-1,5-6"), SIZE), Requested::Full);
        assert_eq!(evaluate(Some("bytes=9-2"), SIZE), Requested::Full);
        assert_eq!(evaluate(Some("bytes=abc-"), SIZE), Requested::Full);
        assert_eq!(evaluate(Some("items=0-1"), SIZE), Requested::Full);
    }

    #[test]
    fn if_range_matches_etag_or_date() {
        assert!(if_range_matches("\"v1-10\"", Some("\"v1-10\""), "Wed"));
        assert!(if_range_matches("Wed", Some("\"v1-10\""), "Wed"));
        assert!(!if_range_matches("\"v2-10\"", Some("\"v1-10\""), "Wed"));
    }
}
