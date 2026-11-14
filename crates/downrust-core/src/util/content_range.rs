// SPDX-License-Identifier: GPL-3.0-or-later
//! Content-Range header parsing.

/// A byte range from a `Content-Range` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    pub end_inclusive: u64,
    pub total: Option<u64>,
}

/// Parses `bytes a-b/N` and `bytes a-b/*`.
pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let (range, total) = bytes_spec(value)?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end_inclusive: u64 = end.trim().parse().ok()?;
    if start > end_inclusive {
        return None;
    }
    let total = match total.trim() {
        "*" => None,
        text => Some(text.parse().ok()?),
    };
    Some(ContentRange {
        start,
        end_inclusive,
        total,
    })
}

/// Parses `bytes */N`, the total size sent with a 416 reply.
pub fn parse_unsatisfied_total(value: &str) -> Option<u64> {
    bytes_spec(value)?.strip_prefix("*/")?.trim().parse().ok()
}

fn bytes_spec(value: &str) -> Option<&str> {
    let (unit, rest) = value.trim().split_once(' ')?;
    unit.eq_ignore_ascii_case("bytes").then_some(rest.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_range_with_total() {
        assert_eq!(
            parse_content_range("bytes 0-0/1000"),
            Some(ContentRange {
                start: 0,
                end_inclusive: 0,
                total: Some(1000)
            })
        );
    }

    #[test]
    fn parses_range_with_unknown_total() {
        assert_eq!(
            parse_content_range("bytes 100-199/*"),
            Some(ContentRange {
                start: 100,
                end_inclusive: 199,
                total: None
            })
        );
    }

    #[test]
    fn tolerates_whitespace_and_unit_case() {
        let parsed = parse_content_range("  Bytes 5 - 9 / 20 ").expect("valid header");
        assert_eq!(parsed.start, 5);
        assert_eq!(parsed.end_inclusive, 9);
        assert_eq!(parsed.total, Some(20));
    }

    #[test]
    fn rejects_invalid_ranges() {
        assert_eq!(parse_content_range("bytes */1000"), None);
        assert_eq!(parse_content_range("bytes 5-3/10"), None);
        assert_eq!(parse_content_range("bytes 0-0/abc"), None);
        assert_eq!(parse_content_range("items 0-1/2"), None);
        assert_eq!(parse_content_range("bytes 0-1"), None);
        assert_eq!(parse_content_range(""), None);
    }

    #[test]
    fn parses_unsatisfied_total() {
        assert_eq!(parse_unsatisfied_total("bytes */1000"), Some(1000));
        assert_eq!(parse_unsatisfied_total("bytes 0-0/1000"), None);
        assert_eq!(parse_unsatisfied_total("bytes */x"), None);
        assert_eq!(parse_unsatisfied_total("items */1000"), None);
    }
}
