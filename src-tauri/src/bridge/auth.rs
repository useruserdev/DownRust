// SPDX-License-Identifier: GPL-3.0-or-later
//! Request checks for the bridge: browser origin, shared token and content type.
use axum::http::StatusCode;

pub const TOKEN_HEADER: &str = "x-downrust-token";

/// Rejects web pages: any origin other than a browser extension gets 403.
/// A request without an origin is let through (some browsers omit it for extension requests);
/// such a request still needs the token, which a web page cannot attach without sending an origin.
pub fn check_origin(origin: Option<&str>) -> Result<(), StatusCode> {
    match origin {
        None => Ok(()),
        Some(value)
            if value.starts_with("chrome-extension://")
                || value.starts_with("moz-extension://") =>
        {
            Ok(())
        }
        Some(_) => Err(StatusCode::FORBIDDEN),
    }
}

/// Compares the token sent by the client with the stored token in constant time.
pub fn check_token(provided: Option<&str>, expected: &str) -> Result<(), StatusCode> {
    let Some(provided) = provided else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let a = provided.as_bytes();
    let b = expected.as_bytes();
    let same_length = a.len() == b.len();
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    if same_length && diff == 0 && !expected.is_empty() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Requires `application/json`; parameters such as `charset` are allowed.
pub fn check_json(content_type: Option<&str>) -> Result<(), StatusCode> {
    let essence = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if essence.eq_ignore_ascii_case("application/json") {
        Ok(())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_rejects_web_pages() {
        assert!(check_origin(Some("chrome-extension://abcdef")).is_ok());
        assert!(check_origin(Some("moz-extension://1234-5678")).is_ok());
        assert!(check_origin(None).is_ok());
        assert_eq!(check_origin(Some("null")), Err(StatusCode::FORBIDDEN));
        assert_eq!(
            check_origin(Some("https://example.com")),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            check_origin(Some("http://localhost:1420")),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn token_must_match_exactly() {
        assert!(check_token(Some("secret"), "secret").is_ok());
        assert_eq!(check_token(None, "secret"), Err(StatusCode::UNAUTHORIZED));
        assert_eq!(
            check_token(Some("secre"), "secret"),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            check_token(Some("secreT"), "secret"),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(check_token(Some(""), ""), Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn content_type_must_be_json() {
        assert!(check_json(Some("application/json")).is_ok());
        assert!(check_json(Some("Application/JSON; charset=utf-8")).is_ok());
        assert_eq!(check_json(None), Err(StatusCode::UNSUPPORTED_MEDIA_TYPE));
        assert_eq!(
            check_json(Some("text/plain")),
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        );
    }
}
