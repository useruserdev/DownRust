// SPDX-License-Identifier: GPL-3.0-or-later
//! Proxy settings, site logins and the host patterns that both use.
use serde::{Deserialize, Serialize};

const PROXY_SCHEMES: &[&str] = &["http", "https", "socks5", "socks5h"];

/// Which proxy the engine uses. `System` follows the Windows proxy settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    #[default]
    System,
    Direct,
    Manual,
}

/// What a proxy rule does with a matching host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyRuleAction {
    Proxy,
    Direct,
}

/// One host rule of a manual proxy. The host is an exact name or `*.example.com`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRule {
    pub host: String,
    pub action: ProxyRuleAction,
}

/// Proxy configuration. The password is not part of it; it lives in the credential store.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    /// Address such as `http://host:port` or `socks5://host:port`.
    pub url: String,
    pub username: String,
    pub rules: Vec<ProxyRule>,
}

/// A user name for HTTP Basic sign-in at a host. The password lives in the credential store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteLogin {
    pub host: String,
    pub username: String,
}

/// The route a request takes for one host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Proxy,
    Direct,
    System,
}

/// Reports whether a host pattern matches a host. Matching ignores case. A pattern
/// `*.example.com` matches `example.com` and every subdomain of it.
pub fn host_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let host = host.trim().to_ascii_lowercase();
    if pattern.is_empty() || host.is_empty() {
        return false;
    }
    match pattern.strip_prefix("*.") {
        Some(domain) => host == domain || host.ends_with(&format!(".{domain}")),
        None => host == pattern,
    }
}

/// Picks the route for a host. Rules apply only in `Manual` mode. In `System` mode the
/// rules are ignored, because the system proxy setting decides the route.
pub fn route_for(settings: &ProxySettings, host: &str) -> Route {
    match settings.mode {
        ProxyMode::System => Route::System,
        ProxyMode::Direct => Route::Direct,
        ProxyMode::Manual => settings
            .rules
            .iter()
            .find(|rule| host_matches(&rule.host, host))
            .map_or(Route::Proxy, |rule| match rule.action {
                ProxyRuleAction::Proxy => Route::Proxy,
                ProxyRuleAction::Direct => Route::Direct,
            }),
    }
}

/// True when the text is an address the engine can use as a proxy.
pub fn valid_proxy_url(text: &str) -> bool {
    url::Url::parse(text.trim())
        .is_ok_and(|parsed| PROXY_SCHEMES.contains(&parsed.scheme()) && parsed.host_str().is_some())
}

impl ProxySettings {
    /// Trims the text fields and drops rules without a host. A manual mode with an invalid
    /// address falls back to `System`, and a warning is logged.
    pub fn sanitized(mut self) -> Self {
        self.url = self.url.trim().to_owned();
        self.username = self.username.trim().to_owned();
        self.rules.retain(|rule| !rule.host.trim().is_empty());
        for rule in &mut self.rules {
            rule.host = rule.host.trim().to_ascii_lowercase();
        }
        if self.mode == ProxyMode::Manual && !valid_proxy_url(&self.url) {
            tracing::warn!("the proxy address is not valid; the system proxy is used instead");
            self.mode = ProxyMode::System;
        }
        self
    }
}

/// Trims and lower-cases the hosts and drops logins without a host.
pub fn sanitized_logins(logins: Vec<SiteLogin>) -> Vec<SiteLogin> {
    logins
        .into_iter()
        .filter_map(|mut login| {
            login.host = login.host.trim().to_ascii_lowercase();
            (!login.host.is_empty()).then_some(login)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manual(rules: Vec<ProxyRule>) -> ProxySettings {
        ProxySettings {
            mode: ProxyMode::Manual,
            url: "http://127.0.0.1:8080".to_owned(),
            username: String::new(),
            rules,
        }
    }

    fn rule(host: &str, action: ProxyRuleAction) -> ProxyRule {
        ProxyRule {
            host: host.to_owned(),
            action,
        }
    }

    #[test]
    fn exact_host_and_wildcard_patterns() {
        assert!(host_matches("example.com", "EXAMPLE.com"));
        assert!(!host_matches("example.com", "sub.example.com"));
        assert!(host_matches("*.example.com", "example.com"));
        assert!(host_matches("*.example.com", "a.b.example.com"));
        assert!(!host_matches("*.example.com", "badexample.com"));
        assert!(!host_matches("", "example.com"));
    }

    #[test]
    fn manual_rules_pick_first_match_and_default_to_proxy() {
        let settings = manual(vec![
            rule("127.0.0.1", ProxyRuleAction::Direct),
            rule("*.local", ProxyRuleAction::Proxy),
        ]);
        assert_eq!(route_for(&settings, "127.0.0.1"), Route::Direct);
        assert_eq!(route_for(&settings, "nas.local"), Route::Proxy);
        assert_eq!(route_for(&settings, "example.com"), Route::Proxy);
    }

    #[test]
    fn direct_and_system_modes_ignore_rules() {
        let mut settings = manual(vec![rule("example.com", ProxyRuleAction::Proxy)]);
        settings.mode = ProxyMode::Direct;
        assert_eq!(route_for(&settings, "example.com"), Route::Direct);
        settings.mode = ProxyMode::System;
        assert_eq!(route_for(&settings, "example.com"), Route::System);
    }

    #[test]
    fn only_four_schemes_are_valid() {
        assert!(valid_proxy_url("socks5h://10.0.0.1:1080"));
        assert!(valid_proxy_url("https://proxy.example.com:443"));
        assert!(!valid_proxy_url("ftp://proxy.example.com"));
        assert!(!valid_proxy_url("not a url"));
    }

    #[test]
    fn invalid_manual_address_falls_back_to_system() {
        let settings = ProxySettings {
            mode: ProxyMode::Manual,
            url: "ftp://nope".to_owned(),
            ..ProxySettings::default()
        };
        assert_eq!(settings.sanitized().mode, ProxyMode::System);
    }

    #[test]
    fn logins_are_normalized_and_empty_hosts_dropped() {
        let logins = sanitized_logins(vec![
            SiteLogin {
                host: " Files.Example.COM ".to_owned(),
                username: "u".to_owned(),
            },
            SiteLogin {
                host: "  ".to_owned(),
                username: "x".to_owned(),
            },
        ]);
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].host, "files.example.com");
    }
}
