// SPDX-License-Identifier: GPL-3.0-or-later
//! Checks and normalizes SHA-256 values written by users.

/// Length of a hex-encoded SHA-256 value.
const HEX_LEN: usize = 64;

/// Trims and lowercases a SHA-256 value. Returns `None` when the text is not 64 hexadecimal
/// characters.
pub fn normalize_sha256(input: &str) -> Option<String> {
    let text = input.trim();
    let valid = text.len() == HEX_LEN && text.bytes().all(|b| b.is_ascii_hexdigit());
    valid.then(|| text.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::normalize_sha256;

    const GOOD: &str = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD";

    #[test]
    fn accepts_and_lowercases_64_hex_characters() {
        assert_eq!(
            normalize_sha256(GOOD).as_deref(),
            Some(GOOD.to_ascii_lowercase().as_str())
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let padded = format!("  {GOOD}\n");
        assert_eq!(
            normalize_sha256(&padded).as_deref(),
            Some(GOOD.to_ascii_lowercase().as_str())
        );
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert_eq!(normalize_sha256(""), None);
        assert_eq!(normalize_sha256(&GOOD[..63]), None);
        assert_eq!(normalize_sha256(&format!("{GOOD}0")), None);
        let bad = format!("{}g", &GOOD[..63]);
        assert_eq!(normalize_sha256(&bad), None);
    }
}
