// SPDX-License-Identifier: GPL-3.0-or-later
//! Parses FTP, FTPS and SFTP addresses and picks the login for them.
use percent_encoding::percent_decode_str;
use url::Url;

use crate::auth;
use crate::error::{Error, Result};
use crate::secrets;
use crate::settings::EngineSettings;
use crate::util::filename::file_name_from_url;

const ANONYMOUS_USER: &str = "anonymous";
const ANONYMOUS_PASSWORD: &str = "anonymous@";
const FALLBACK_NAME: &str = "download";

/// Message when an SFTP address has no saved user name and password.
pub(crate) const SFTP_LOGIN_NEEDED: &str =
    "This SFTP server needs a user name and password. Add them in Settings → Sites.";

/// The protocol of an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Protocol {
    Ftp,
    Ftps,
    Sftp,
}

/// The parts of an FTP, FTPS or SFTP address that the transfer needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Address {
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    /// Absolute path on the server, percent-decoded.
    pub path: String,
    /// Last path segment, decoded and sanitized.
    pub file_name: String,
    /// User name written in the address, decoded.
    pub user: Option<String>,
    /// Password written in the address, decoded.
    pub password: Option<String>,
}

/// User name and password for the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Login {
    pub user: String,
    pub password: Option<String>,
}

fn decode(text: &str) -> String {
    percent_decode_str(text).decode_utf8_lossy().into_owned()
}

impl Address {
    /// Parses an `ftp://`, `ftps://` or `sftp://` address.
    pub fn parse(text: &str) -> Result<Self> {
        let url = Url::parse(text.trim()).map_err(|err| Error::InvalidUrl(err.to_string()))?;
        let (protocol, default_port) = match url.scheme() {
            "ftp" => (Protocol::Ftp, 21),
            "ftps" => (Protocol::Ftps, 21),
            "sftp" => (Protocol::Sftp, 22),
            other => return Err(Error::InvalidUrl(format!("unsupported scheme {other}"))),
        };
        let host = url
            .host_str()
            .map(|host| {
                host.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_owned()
            })
            .filter(|host| !host.is_empty())
            .ok_or_else(|| Error::InvalidUrl("the address has no host name".to_owned()))?;
        let user = Some(decode(url.username())).filter(|name| !name.is_empty());
        let password = url.password().map(decode);
        let path = decode(url.path());
        let file_name = file_name_from_url(&url).unwrap_or_else(|| FALLBACK_NAME.to_owned());
        Ok(Self {
            protocol,
            host,
            port: url.port().unwrap_or(default_port),
            path,
            file_name,
            user,
            password,
        })
    }
}

/// Reads the saved password of a site. A store error counts as no password.
fn saved_password(host: &str) -> Option<String> {
    secrets::get(&secrets::site_user(host)).unwrap_or_else(|err| {
        tracing::warn!("cannot read the password for {host}: {err}");
        None
    })
}

/// Picks the login: the one in the address, else the saved site login, else anonymous for FTP.
/// SFTP needs a user name and a password from one of the first two sources.
pub(crate) fn resolve_login(
    address: &Address,
    text: &str,
    settings: &EngineSettings,
) -> Result<Login> {
    if let Some(user) = &address.user {
        return Ok(Login {
            user: user.clone(),
            password: address.password.clone(),
        });
    }
    let saved = auth::login_for(settings, text).map(|site| Login {
        user: site.username.clone(),
        password: saved_password(&site.host),
    });
    match (address.protocol, saved) {
        (
            Protocol::Sftp,
            Some(Login {
                user,
                password: Some(password),
            }),
        ) if !password.is_empty() => Ok(Login {
            user,
            password: Some(password),
        }),
        (Protocol::Sftp, _) => Err(Error::Transfer(SFTP_LOGIN_NEEDED.to_owned())),
        (_, Some(login)) => Ok(login),
        (_, None) => Ok(Login {
            user: ANONYMOUS_USER.to_owned(),
            password: Some(ANONYMOUS_PASSWORD.to_owned()),
        }),
    }
}

/// File name used when the request gives none.
pub(crate) fn initial_file_name(text: &str) -> String {
    Address::parse(text)
        .map(|address| address.file_name)
        .unwrap_or_else(|_| FALLBACK_NAME.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::filename::sanitize_file_name;

    #[test]
    fn anonymous_ftp_address_uses_default_port_and_decoded_path() {
        let address = Address::parse("ftp://files.example.com/pub/a%20b.zip").expect("parses");
        assert_eq!(address.protocol, Protocol::Ftp);
        assert_eq!(address.host, "files.example.com");
        assert_eq!(address.port, 21);
        assert_eq!(address.path, "/pub/a b.zip");
        assert_eq!(address.file_name, "a b.zip");
        assert_eq!(address.user, None);
        assert_eq!(address.password, None);
    }

    #[test]
    fn credentials_and_port_come_from_the_address() {
        let address =
            Address::parse("ftp://us%40er:p%2Fss@host.example:2121/x.bin").expect("parses");
        assert_eq!(address.port, 2121);
        assert_eq!(address.user.as_deref(), Some("us@er"));
        assert_eq!(address.password.as_deref(), Some("p/ss"));
        assert_eq!(address.file_name, "x.bin");
    }

    #[test]
    fn ftps_and_sftp_have_their_own_default_ports() {
        let ftps = Address::parse("ftps://h.example/a").expect("parses");
        assert_eq!(ftps.protocol, Protocol::Ftps);
        assert_eq!(ftps.port, 21);
        let sftp = Address::parse("sftp://h.example/a").expect("parses");
        assert_eq!(sftp.protocol, Protocol::Sftp);
        assert_eq!(sftp.port, 22);
    }

    #[test]
    fn name_is_sanitized_last_segment() {
        let address = Address::parse("sftp://h.example/dir/bad%3Aname.txt").expect("parses");
        assert_eq!(address.file_name, sanitize_file_name("bad:name.txt"));
        assert!(!address.file_name.contains(':'));
    }

    #[test]
    fn missing_last_segment_falls_back_to_download() {
        let address = Address::parse("ftp://h.example/").expect("parses");
        assert_eq!(address.path, "/");
        assert_eq!(address.file_name, "download");
        assert_eq!(initial_file_name("ftp://h.example/"), "download");
    }

    #[test]
    fn unknown_scheme_and_missing_host_are_rejected() {
        assert!(Address::parse("gopher://h.example/a").is_err());
        assert!(Address::parse("ftp:///a").is_err());
    }

    #[test]
    fn ftp_without_login_is_anonymous() {
        let address = Address::parse("ftp://anon-unit-one.example/a.bin").expect("parses");
        let login = resolve_login(
            &address,
            "ftp://anon-unit-one.example/a.bin",
            &EngineSettings::default(),
        )
        .expect("login");
        assert_eq!(login.user, "anonymous");
        assert_eq!(login.password.as_deref(), Some("anonymous@"));
    }

    #[test]
    fn sftp_without_password_asks_for_login() {
        let text = "sftp://sftp-unit-one.example/a.bin";
        let address = Address::parse(text).expect("parses");
        match resolve_login(&address, text, &EngineSettings::default()) {
            Err(Error::Transfer(msg)) => assert_eq!(msg, SFTP_LOGIN_NEEDED),
            other => panic!("expected the login message, got {other:?}"),
        }
    }

    #[test]
    fn saved_site_login_supplies_user_and_password() {
        secrets::use_memory_backend();
        secrets::set(&secrets::site_user("ftp-unit-site.example"), Some("pw-1")).expect("set");
        let settings = EngineSettings {
            site_logins: vec![crate::settings::SiteLogin {
                host: "ftp-unit-site.example".to_owned(),
                username: "site-user".to_owned(),
            }],
            ..EngineSettings::default()
        };
        let text = "ftp://ftp-unit-site.example/a.bin";
        let address = Address::parse(text).expect("parses");
        let login = resolve_login(&address, text, &settings).expect("login");
        assert_eq!(login.user, "site-user");
        assert_eq!(login.password.as_deref(), Some("pw-1"));
    }
}
