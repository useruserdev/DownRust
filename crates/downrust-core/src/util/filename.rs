// SPDX-License-Identifier: GPL-3.0-or-later
//! File name sanitizing and uniqueness helpers.
use percent_encoding::percent_decode_str;
use url::Url;

const MAX_NAME_CHARS: usize = 180;
const FALLBACK_NAME: &str = "download";

/// Makes a name safe for Windows file systems.
pub fn sanitize_file_name(name: &str) -> String {
    let replaced: String = name.chars().map(replace_forbidden).collect();
    let trimmed = trim_tail(replaced.trim_start_matches(' '));
    let capped = cap_length(&guard_reserved(trimmed));
    let finished = cap_length(&guard_reserved(&capped));
    if finished.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        finished
    }
}

/// Returns the last non-empty path segment of the address, decoded and sanitized.
pub fn file_name_from_url(url: &Url) -> Option<String> {
    let mut segments = url.path_segments()?;
    let segment = segments.rfind(|s| !s.is_empty())?;
    let decoded = percent_decode_str(segment).decode_utf8_lossy();
    if decoded.trim().is_empty() {
        return None;
    }
    Some(sanitize_file_name(&decoded))
}

/// Splits a name at its last dot. A leading dot or a trailing dot does not start an extension.
pub fn split_extension(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(index) if index > 0 && index + 1 < name.len() => {
            (&name[..index], Some(&name[index + 1..]))
        }
        _ => (name, None),
    }
}

/// Returns `desired` when it is free, otherwise the first free "stem (n).ext" name.
pub fn unique_file_name(desired: &str, is_taken: impl Fn(&str) -> bool) -> String {
    if !is_taken(desired) {
        return desired.to_owned();
    }
    let (stem, extension) = split_extension(desired);
    let mut number = 1_u32;
    loop {
        let candidate = match extension {
            Some(ext) => format!("{stem} ({number}).{ext}"),
            None => format!("{desired} ({number})"),
        };
        if !is_taken(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

fn replace_forbidden(c: char) -> char {
    if c < ' ' || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
        '_'
    } else {
        c
    }
}

fn trim_tail(name: &str) -> &str {
    name.trim_end_matches(['.', ' '])
}

fn guard_reserved(name: &str) -> String {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if is_reserved_stem(&stem) {
        format!("_{name}")
    } else {
        name.to_owned()
    }
}

fn is_reserved_stem(stem: &str) -> bool {
    if matches!(stem, "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let digit = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    matches!(
        digit,
        Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
    )
}

fn cap_length(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_CHARS {
        return name.to_owned();
    }
    let capped: String = match split_extension(name) {
        (stem, Some(ext)) if ext.chars().count() + 2 <= MAX_NAME_CHARS => {
            let keep = MAX_NAME_CHARS - ext.chars().count() - 1;
            let short_stem: String = stem.chars().take(keep).collect();
            format!("{short_stem}.{ext}")
        }
        _ => name.chars().take(MAX_NAME_CHARS).collect(),
    };
    trim_tail(&capped).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_forbidden_characters() {
        assert_eq!(
            sanitize_file_name("a<b>c:d\"e/f\\g|h?i*j.txt"),
            "a_b_c_d_e_f_g_h_i_j.txt"
        );
    }

    #[test]
    fn replaces_control_characters() {
        assert_eq!(sanitize_file_name("tab\there\nend.txt"), "tab_here_end.txt");
    }

    #[test]
    fn trims_leading_spaces_and_trailing_dots_and_spaces() {
        assert_eq!(sanitize_file_name("  leading.txt"), "leading.txt");
        assert_eq!(sanitize_file_name("trailing.txt. "), "trailing.txt");
        assert_eq!(sanitize_file_name("name . ."), "name");
    }

    #[test]
    fn prefixes_reserved_device_names() {
        assert_eq!(sanitize_file_name("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_file_name("con"), "_con");
        assert_eq!(sanitize_file_name("COM1"), "_COM1");
        assert_eq!(sanitize_file_name("lpt9.log"), "_lpt9.log");
        assert_eq!(sanitize_file_name("NUL"), "_NUL");
    }

    #[test]
    fn keeps_names_that_only_look_reserved() {
        assert_eq!(sanitize_file_name("COM10.txt"), "COM10.txt");
        assert_eq!(sanitize_file_name("COM0.txt"), "COM0.txt");
        assert_eq!(sanitize_file_name("CONSOLE.txt"), "CONSOLE.txt");
    }

    #[test]
    fn empty_results_become_download() {
        assert_eq!(sanitize_file_name(""), "download");
        assert_eq!(sanitize_file_name("   "), "download");
        assert_eq!(sanitize_file_name("..."), "download");
    }

    #[test]
    fn caps_length_at_180_chars() {
        let result = sanitize_file_name(&"a".repeat(300));
        assert_eq!(result.chars().count(), 180);
    }

    #[test]
    fn caps_length_and_keeps_extension() {
        let result = sanitize_file_name(&format!("{}.pdf", "b".repeat(300)));
        assert_eq!(result.chars().count(), 180);
        assert!(result.ends_with(".pdf"));
        assert!(result.starts_with("bbb"));
    }

    #[test]
    fn truncation_cannot_create_a_reserved_name() {
        let result = sanitize_file_name(&format!("COM1xxxx.{}", "e".repeat(175)));
        assert_eq!(result.chars().count(), 180);
        assert!(result.starts_with('_'));
    }

    #[test]
    fn keeps_unicode_names() {
        assert_eq!(sanitize_file_name("résumé.pdf"), "résumé.pdf");
    }

    #[test]
    fn file_name_from_url_takes_last_segment_and_decodes() {
        let url =
            Url::parse("https://example.com/files/My%20File%3F.zip?x=1#frag").expect("valid url");
        assert_eq!(file_name_from_url(&url).as_deref(), Some("My File_.zip"));
    }

    #[test]
    fn file_name_from_url_returns_none_without_segment() {
        let url = Url::parse("https://example.com/").expect("valid url");
        assert_eq!(file_name_from_url(&url), None);
    }

    #[test]
    fn file_name_from_url_decodes_utf8_and_guards_reserved() {
        let euro = Url::parse("https://example.com/%E2%82%AC%20rates.csv").expect("valid url");
        assert_eq!(file_name_from_url(&euro).as_deref(), Some("€ rates.csv"));
        let reserved = Url::parse("https://example.com/dl/CON").expect("valid url");
        assert_eq!(file_name_from_url(&reserved).as_deref(), Some("_CON"));
    }

    #[test]
    fn split_extension_uses_last_dot_and_ignores_leading_dot() {
        assert_eq!(split_extension("a.b.c"), ("a.b", Some("c")));
        assert_eq!(
            split_extension("archive.tar.gz"),
            ("archive.tar", Some("gz"))
        );
        assert_eq!(split_extension(".bashrc"), (".bashrc", None));
        assert_eq!(split_extension("noext"), ("noext", None));
        assert_eq!(split_extension("trailing."), ("trailing.", None));
    }

    #[test]
    fn unique_name_returns_free_name_unchanged() {
        assert_eq!(unique_file_name("a.txt", |_| false), "a.txt");
    }

    #[test]
    fn unique_name_adds_first_free_number() {
        let taken = ["a.txt", "a (1).txt"];
        let result = unique_file_name("a.txt", |n| taken.contains(&n));
        assert_eq!(result, "a (2).txt");
    }

    #[test]
    fn unique_name_without_extension() {
        let result = unique_file_name("data", |n| n == "data");
        assert_eq!(result, "data (1)");
    }

    #[test]
    fn unique_name_check_can_be_case_insensitive() {
        let result = unique_file_name("a.txt", |n| n.eq_ignore_ascii_case("A.TXT"));
        assert_eq!(result, "a (1).txt");
    }
}
