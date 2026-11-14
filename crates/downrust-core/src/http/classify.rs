// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP response classification.

/// How the engine should react to a status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusClass {
    Success,
    Busy,
    Retryable,
    Fatal,
}

/// 2xx is success. 429 and 503 mean busy. 408 and other 5xx are retryable. Anything else is fatal.
pub fn classify_status(code: u16) -> StatusClass {
    match code {
        200..=299 => StatusClass::Success,
        429 | 503 => StatusClass::Busy,
        408 | 500..=599 => StatusClass::Retryable,
        _ => StatusClass::Fatal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_hundreds_are_success() {
        assert_eq!(classify_status(200), StatusClass::Success);
        assert_eq!(classify_status(204), StatusClass::Success);
        assert_eq!(classify_status(206), StatusClass::Success);
        assert_eq!(classify_status(299), StatusClass::Success);
    }

    #[test]
    fn busy_codes() {
        assert_eq!(classify_status(429), StatusClass::Busy);
        assert_eq!(classify_status(503), StatusClass::Busy);
    }

    #[test]
    fn retryable_codes() {
        assert_eq!(classify_status(408), StatusClass::Retryable);
        assert_eq!(classify_status(500), StatusClass::Retryable);
        assert_eq!(classify_status(502), StatusClass::Retryable);
        assert_eq!(classify_status(504), StatusClass::Retryable);
        assert_eq!(classify_status(599), StatusClass::Retryable);
    }

    #[test]
    fn everything_else_is_fatal() {
        assert_eq!(classify_status(0), StatusClass::Fatal);
        assert_eq!(classify_status(300), StatusClass::Fatal);
        assert_eq!(classify_status(400), StatusClass::Fatal);
        assert_eq!(classify_status(404), StatusClass::Fatal);
        assert_eq!(classify_status(416), StatusClass::Fatal);
        assert_eq!(classify_status(600), StatusClass::Fatal);
    }
}
