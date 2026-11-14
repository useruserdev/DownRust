// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP Basic logins for sites, and the messages shown when a site refuses access.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::model::HeaderPair;
use crate::secrets;
use crate::settings::{host_matches, EngineSettings, SiteLogin};

/// Message when a site asks for a login and none is saved for it.
pub const LOGIN_NEEDED: &str = "This site needs a login. Add it in Settings → Sites.";

/// Host part of an address, or an empty text when the address has none.
pub fn request_host(address: &str) -> String {
    url::Url::parse(address.trim())
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The first saved login whose host pattern matches the address.
pub fn login_for<'a>(settings: &'a EngineSettings, address: &str) -> Option<&'a SiteLogin> {
    let parsed = url::Url::parse(address.trim()).ok()?;
    let host = parsed.host_str()?;
    settings
        .site_logins
        .iter()
        .find(|login| host_matches(&login.host, host))
}

/// Returns the headers with an `Authorization` header added when a saved login matches.
/// The password is read from the credential store. A store error counts as no password.
pub fn with_site_login(
    settings: &EngineSettings,
    address: &str,
    headers: &[HeaderPair],
) -> Vec<HeaderPair> {
    let Some(login) = login_for(settings, address) else {
        return headers.to_vec();
    };
    let mut out: Vec<HeaderPair> = headers
        .iter()
        .filter(|header| !header.name.eq_ignore_ascii_case("authorization"))
        .cloned()
        .collect();
    let password = secrets::get(&secrets::site_user(&login.host))
        .unwrap_or_else(|err| {
            tracing::warn!("cannot read the password for {}: {err}", login.host);
            None
        })
        .unwrap_or_default();
    let token = STANDARD.encode(format!("{}:{}", login.username, password));
    out.push(HeaderPair {
        name: "Authorization".to_owned(),
        value: format!("Basic {token}"),
    });
    out
}

/// Message for a `401` reply. A saved login was refused, or no login exists.
pub fn unauthorized_message(has_login: bool, host: &str) -> String {
    if has_login {
        format!("The site rejected the login for {host}.")
    } else {
        LOGIN_NEEDED.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(host: &str) -> EngineSettings {
        EngineSettings {
            site_logins: vec![SiteLogin {
                host: host.to_owned(),
                username: "demo".to_owned(),
            }],
            ..EngineSettings::default()
        }
    }

    #[test]
    fn login_matches_by_host_pattern() {
        let settings = settings_with("*.example.com");
        assert!(login_for(&settings, "https://files.example.com/a").is_some());
        assert!(login_for(&settings, "https://other.org/a").is_none());
    }

    #[test]
    fn matching_login_adds_basic_header() {
        secrets::use_memory_backend();
        secrets::set(&secrets::site_user("auth-unit.example"), Some("secret")).expect("set");
        let settings = settings_with("auth-unit.example");
        let headers = with_site_login(&settings, "http://auth-unit.example/x", &[]);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].name, "Authorization");
        assert_eq!(headers[0].value, "Basic ZGVtbzpzZWNyZXQ=");
    }

    #[test]
    fn no_matching_login_leaves_headers_alone() {
        let settings = settings_with("elsewhere.example");
        let extra = [HeaderPair {
            name: "Referer".to_owned(),
            value: "https://a.example/".to_owned(),
        }];
        assert_eq!(
            with_site_login(&settings, "http://127.0.0.1/x", &extra),
            extra
        );
    }

    #[test]
    fn messages_depend_on_saved_login() {
        assert_eq!(unauthorized_message(false, "h"), LOGIN_NEEDED);
        assert_eq!(
            unauthorized_message(true, "files.example.com"),
            "The site rejected the login for files.example.com."
        );
    }
}
