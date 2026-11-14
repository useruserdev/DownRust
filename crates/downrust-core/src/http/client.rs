// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP client construction.
use std::time::Duration;

use reqwest::ClientBuilder;

use crate::error::Result;
use crate::secrets;
use crate::settings::{route_for, EngineSettings, ProxyMode, Route};

const MAX_REDIRECTS: usize = 10;
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Builds the shared HTTP client from the engine settings. No whole-request timeout is set,
/// because long downloads must not be cut off.
pub fn build_client(settings: &EngineSettings) -> Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .user_agent(settings.user_agent.as_str())
        .connect_timeout(Duration::from_secs(settings.connect_timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .tcp_nodelay(true);
    Ok(apply_proxy(builder, settings).build()?)
}

/// The proxy mode in effect. A `System` mode with `use_system_proxy` off means no proxy.
fn effective_mode(settings: &EngineSettings) -> ProxyMode {
    match settings.proxy.mode {
        ProxyMode::System if !settings.use_system_proxy => ProxyMode::Direct,
        mode => mode,
    }
}

fn apply_proxy(builder: ClientBuilder, settings: &EngineSettings) -> ClientBuilder {
    match effective_mode(settings) {
        ProxyMode::System => builder,
        ProxyMode::Direct => builder.no_proxy(),
        ProxyMode::Manual => match proxy_address(settings) {
            Some(address) => {
                let proxy = settings.proxy.clone();
                builder
                    .no_proxy()
                    .proxy(reqwest::Proxy::custom(move |target| {
                        let host = target.host_str().unwrap_or_default();
                        match route_for(&proxy, host) {
                            Route::Proxy => Some(address.clone()),
                            Route::Direct | Route::System => None,
                        }
                    }))
            }
            None => builder.no_proxy(),
        },
    }
}

/// The manual proxy address with the user name and the stored password.
fn proxy_address(settings: &EngineSettings) -> Option<url::Url> {
    let mut address = url::Url::parse(&settings.proxy.url).ok()?;
    let user = settings.proxy.username.as_str();
    if !user.is_empty() {
        let _ = address.set_username(user);
        let password = secrets::get(secrets::PROXY_USER).unwrap_or_else(|err| {
            tracing::warn!("cannot read the proxy password: {err}");
            None
        });
        let _ = address.set_password(password.as_deref());
    }
    Some(address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ProxyRule, ProxyRuleAction, ProxySettings};

    #[test]
    fn builds_with_default_settings() {
        assert!(build_client(&EngineSettings::default()).is_ok());
    }

    #[test]
    fn builds_without_system_proxy() {
        let settings = EngineSettings {
            use_system_proxy: false,
            ..EngineSettings::default()
        };
        assert!(build_client(&settings).is_ok());
    }

    #[test]
    fn builds_with_manual_proxy_and_rules() {
        let settings = EngineSettings {
            proxy: ProxySettings {
                mode: ProxyMode::Manual,
                url: "socks5h://127.0.0.1:1080".to_owned(),
                username: "user".to_owned(),
                rules: vec![ProxyRule {
                    host: "*.lan".to_owned(),
                    action: ProxyRuleAction::Direct,
                }],
            },
            ..EngineSettings::default()
        };
        assert!(build_client(&settings).is_ok());
    }

    #[test]
    fn disabled_system_proxy_maps_to_direct() {
        let settings = EngineSettings {
            use_system_proxy: false,
            ..EngineSettings::default()
        };
        assert_eq!(effective_mode(&settings), ProxyMode::Direct);
    }
}
