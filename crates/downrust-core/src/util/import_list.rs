// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads download addresses from a plain text list, an IDM export (.ief) or a saved web page.
use std::collections::HashSet;

/// Most addresses that one import keeps.
const MAX_ADDRESSES: usize = 10_000;

/// Address schemes that the import accepts, lowercase.
const SCHEMES: [&str; 6] = [
    "http://", "https://", "ftp://", "ftps://", "sftp://", "magnet:",
];

/// Characters that end an address in running text, such as a closing bracket or a full stop.
const TRAILING: &[char] = &[')', '>', ',', '.', ';', '"', '\''];

/// Extracts download addresses from text. Works for plain lists, IDM exports and HTML.
///
/// In an IDM export each block starts with a line `<` and ends with a line `>`. Only the first
/// non-empty line of a block is read, so the referer and other lines of the block are ignored.
/// Elsewhere, each address starts with an accepted scheme and ends at whitespace, a quote or an
/// angle bracket. Keeps the order of first appearance and drops repeats.
pub fn parse_download_list(text: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut found: Vec<String> = Vec::new();
    let mut in_block = false;
    let mut block_wants_address = false;
    for line in text.lines() {
        let trimmed = line.trim();
        let addresses = if trimmed == "<" {
            in_block = true;
            block_wants_address = true;
            Vec::new()
        } else if trimmed == ">" {
            in_block = false;
            Vec::new()
        } else if in_block {
            if block_wants_address && !trimmed.is_empty() {
                block_wants_address = false;
                addresses_in(trimmed)
            } else {
                Vec::new()
            }
        } else {
            addresses_in(line)
        };
        for address in addresses {
            if seen.insert(address.clone()) {
                found.push(address);
                if found.len() >= MAX_ADDRESSES {
                    return found;
                }
            }
        }
    }
    found
}

/// Every address that starts in one line, in order.
fn addresses_in(line: &str) -> Vec<String> {
    // ASCII lowercasing keeps every byte position, so positions found here index `line` too.
    let lower = line.to_ascii_lowercase();
    let mut starts: Vec<(usize, usize)> = Vec::new();
    for scheme in SCHEMES {
        for (pos, _) in lower.match_indices(scheme) {
            starts.push((pos, scheme.len()));
        }
    }
    starts.sort_unstable();

    let mut found = Vec::new();
    let mut next_free = 0;
    for (pos, scheme_len) in starts {
        if pos < next_free {
            continue;
        }
        let rest = &line[pos..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
            .unwrap_or(rest.len());
        next_free = pos + end;
        let raw = rest[..end].replace("&amp;", "&");
        let address = raw.trim_end_matches(TRAILING);
        if address.len() > scheme_len {
            found.push(address.to_owned());
        }
    }
    found
}

/// Turns the bytes of a text file into a string. Handles a UTF-8 byte order mark, and UTF-16 in
/// either byte order when the file starts with its byte order mark. Other input is read as UTF-8.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, u16::from_le_bytes);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, u16::from_be_bytes);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_utf16(bytes: &[u8], to_unit: fn([u8; 2]) -> u16) -> String {
    let (pairs, _) = bytes.as_chunks::<2>();
    let units = pairs.iter().map(|pair| to_unit(*pair));
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_list_with_mixed_lines() {
        let text = "Here are my files:\nhttps://a.example/one.zip\n\n  ftp://b.example/two.iso  \nnot a link\nsftp://c.example/three.bin\n";
        assert_eq!(
            parse_download_list(text),
            vec![
                "https://a.example/one.zip",
                "ftp://b.example/two.iso",
                "sftp://c.example/three.bin",
            ]
        );
    }

    #[test]
    fn idm_export_blocks_read_only_the_address_line() {
        let text = "<\nhttps://a.example/movie.mkv\nreferer: https://a.example/\nlocalpath: C:\\Downloads\n>\n<\n\nftps://b.example/data.7z\nreferer: https://b.example/\n>\n";
        assert_eq!(
            parse_download_list(text),
            vec!["https://a.example/movie.mkv", "ftps://b.example/data.7z"]
        );
    }

    #[test]
    fn html_href_values_with_entities_decoded() {
        let text = r#"<a href="https://a.example/get?id=7&amp;part=2">get</a> <a href='http://b.example/x'>x</a>"#;
        assert_eq!(
            parse_download_list(text),
            vec!["https://a.example/get?id=7&part=2", "http://b.example/x"]
        );
    }

    #[test]
    fn magnet_links_are_kept() {
        let text = "magnet:?xt=urn:btih:abc123&dn=test\n";
        assert_eq!(
            parse_download_list(text),
            vec!["magnet:?xt=urn:btih:abc123&dn=test"]
        );
    }

    #[test]
    fn trailing_punctuation_is_trimmed() {
        let text = "(https://a.example/x.zip), see https://a.example/y.zip.; and \"https://a.example/z.zip\"";
        assert_eq!(
            parse_download_list(text),
            vec![
                "https://a.example/x.zip",
                "https://a.example/y.zip",
                "https://a.example/z.zip",
            ]
        );
    }

    #[test]
    fn repeats_are_dropped_and_order_kept() {
        let text = "https://a.example/2\nhttps://a.example/1\nhttps://a.example/2\n";
        assert_eq!(
            parse_download_list(text),
            vec!["https://a.example/2", "https://a.example/1"]
        );
    }

    #[test]
    fn scheme_inside_another_address_is_not_split() {
        let text = "https://a.example/redirect?to=https://b.example/file";
        assert_eq!(
            parse_download_list(text),
            vec!["https://a.example/redirect?to=https://b.example/file"]
        );
    }

    #[test]
    fn scheme_without_a_path_is_skipped() {
        assert!(parse_download_list("https:// and magnet:").is_empty());
    }

    #[test]
    fn at_most_ten_thousand_addresses() {
        let text: String = (0..10_005)
            .map(|n| format!("https://a.example/{n}\n"))
            .collect();
        let found = parse_download_list(&text);
        assert_eq!(found.len(), 10_000);
        assert_eq!(found[9_999], "https://a.example/9999");
    }

    #[test]
    fn decodes_utf8_with_and_without_bom() {
        assert_eq!(decode_text("привет".as_bytes()), "привет");
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice("https://a.example/ß".as_bytes());
        assert_eq!(decode_text(&with_bom), "https://a.example/ß");
    }

    #[test]
    fn decodes_utf16_little_endian() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "https://a.example/ü".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode_text(&bytes), "https://a.example/ü");
    }

    #[test]
    fn decodes_utf16_big_endian() {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "magnet:?xt=1".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(decode_text(&bytes), "magnet:?xt=1");
    }

    #[test]
    fn utf16_import_finds_addresses() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "https://a.example/1\r\nhttps://a.example/2".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(
            parse_download_list(&decode_text(&bytes)),
            vec!["https://a.example/1", "https://a.example/2"]
        );
    }
}
