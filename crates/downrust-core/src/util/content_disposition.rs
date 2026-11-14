// SPDX-License-Identifier: GPL-3.0-or-later
//! Content-Disposition header parsing.
use percent_encoding::percent_decode;

/// Returns the file name from a Content-Disposition value. Callers decode the raw header with
/// `String::from_utf8_lossy(value.as_bytes())`. The `filename*` parameter wins over `filename`.
/// The result is only the basename and is not sanitized.
pub fn filename_from_content_disposition(value: &str) -> Option<String> {
    let mut plain: Option<&str> = None;
    let mut extended: Option<&str> = None;
    for param in split_params(value) {
        let Some((key, raw)) = param.split_once('=') else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "filename*" if extended.is_none() => extended = Some(raw.trim()),
            "filename" if plain.is_none() => plain = Some(raw.trim()),
            _ => {}
        }
    }
    let candidates = [extended.and_then(decode_extended), plain.map(decode_plain)];
    candidates
        .into_iter()
        .flatten()
        .map(|name| basename(&name))
        .find(|name| !name.is_empty())
}

fn split_params(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, c) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => {
                parts.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

fn decode_extended(raw: &str) -> Option<String> {
    let mut parts = raw.splitn(3, '\'');
    let charset = parts.next()?;
    let _language = parts.next()?;
    let encoded = parts.next()?;
    let bytes: Vec<u8> = percent_decode(encoded.as_bytes()).collect();
    if charset.eq_ignore_ascii_case("iso-8859-1") {
        Some(bytes.iter().map(|&b| char::from(b)).collect())
    } else {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn decode_plain(raw: &str) -> String {
    let unquoted = unquote(raw);
    if unquoted.contains('%') {
        let bytes: Vec<u8> = percent_decode(unquoted.as_bytes()).collect();
        if let Ok(decoded) = String::from_utf8(bytes) {
            return decoded;
        }
    }
    unquoted
}

fn unquote(raw: &str) -> String {
    match raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inner) => unescape(inner),
        None => raw.to_owned(),
    }
}

fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn basename(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quoted_and_unquoted_filename() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"report.pdf\"").as_deref(),
            Some("report.pdf")
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=report.pdf").as_deref(),
            Some("report.pdf")
        );
    }

    #[test]
    fn keys_are_case_insensitive() {
        assert_eq!(
            filename_from_content_disposition("ATTACHMENT; FileName=\"x.zip\"").as_deref(),
            Some("x.zip")
        );
    }

    #[test]
    fn semicolon_inside_quotes_is_not_a_separator() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"a;b.txt\"").as_deref(),
            Some("a;b.txt")
        );
    }

    #[test]
    fn escaped_quote_and_backslash_are_unescaped() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"a\\\"b.txt\"").as_deref(),
            Some("a\"b.txt")
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"x\\\\y.txt\"").as_deref(),
            Some("y.txt")
        );
    }

    #[test]
    fn filename_star_utf8_wins_over_filename() {
        let value = "attachment; filename=\"fallback.pdf\"; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf";
        assert_eq!(
            filename_from_content_disposition(value).as_deref(),
            Some("résumé.pdf")
        );
    }

    #[test]
    fn filename_star_iso_8859_1_maps_bytes_to_chars() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename*=ISO-8859-1''caf%E9.pdf")
                .as_deref(),
            Some("café.pdf")
        );
    }

    #[test]
    fn filename_star_with_euro_sign() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename*=UTF-8''%E2%82%AC%20rates.txt")
                .as_deref(),
            Some("€ rates.txt")
        );
    }

    #[test]
    fn plain_percent_encoded_name_is_decoded() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=r%C3%A9sum%C3%A9.pdf")
                .as_deref(),
            Some("résumé.pdf")
        );
    }

    #[test]
    fn keeps_only_basename() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"../../etc/passwd\"")
                .as_deref(),
            Some("passwd")
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"C:\\\\dir\\\\setup.exe\"")
                .as_deref(),
            Some("setup.exe")
        );
    }

    #[test]
    fn empty_or_missing_name_gives_none() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"\""),
            None
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"/\""),
            None
        );
        assert_eq!(filename_from_content_disposition("inline"), None);
        assert_eq!(filename_from_content_disposition(""), None);
    }

    #[test]
    fn malformed_filename_star_falls_back_to_filename() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"ok.txt\"; filename*=broken")
                .as_deref(),
            Some("ok.txt")
        );
    }
}
