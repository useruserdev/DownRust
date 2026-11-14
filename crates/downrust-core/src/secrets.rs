// SPDX-License-Identifier: GPL-3.0-or-later
//! Passwords kept in the operating system credential store under the service name `DownRust`.
//!
//! Tests keep the passwords in memory instead. Unit tests always do; integration tests call
//! [`use_memory_backend`] first.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::error::{Error, Result};

/// Service name of every stored password.
pub const SERVICE: &str = "DownRust";
/// User name of the proxy password.
pub const PROXY_USER: &str = "proxy";

static MEMORY_BACKEND: AtomicBool = AtomicBool::new(false);
static MEMORY: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// User name under which the password of a site login is stored.
pub fn site_user(host: &str) -> String {
    format!("site:{}", host.trim().to_ascii_lowercase())
}

/// Keeps passwords in memory for the rest of the process instead of the system store.
pub fn use_memory_backend() {
    MEMORY_BACKEND.store(true, Ordering::SeqCst);
}

fn memory_mode() -> bool {
    cfg!(test) || MEMORY_BACKEND.load(Ordering::SeqCst)
}

fn store_error(err: keyring::Error) -> Error {
    Error::Secrets(err.to_string())
}

/// Reads the password stored for `user`. `None` means nothing is stored.
pub fn get(user: &str) -> Result<Option<String>> {
    if memory_mode() {
        let guard = MEMORY.lock().unwrap_or_else(PoisonError::into_inner);
        return Ok(guard.as_ref().and_then(|map| map.get(user).cloned()));
    }
    let entry = keyring::Entry::new(SERVICE, user).map_err(store_error)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(store_error(err)),
    }
}

/// Stores a password for `user`. `None` deletes the stored password.
pub fn set(user: &str, password: Option<&str>) -> Result<()> {
    if memory_mode() {
        let mut guard = MEMORY.lock().unwrap_or_else(PoisonError::into_inner);
        let map = guard.get_or_insert_with(HashMap::new);
        match password {
            Some(text) => {
                map.insert(user.to_owned(), text.to_owned());
            }
            None => {
                map.remove(user);
            }
        }
        return Ok(());
    }
    let entry = keyring::Entry::new(SERVICE, user).map_err(store_error)?;
    match password {
        Some(text) => entry.set_password(text).map_err(store_error),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(store_error(err)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_backend_stores_and_deletes() {
        set("unit-test-user", Some("pw")).expect("set");
        assert_eq!(get("unit-test-user").expect("get").as_deref(), Some("pw"));
        set("unit-test-user", None).expect("delete");
        assert_eq!(get("unit-test-user").expect("get"), None);
    }

    #[test]
    fn site_users_are_lower_case() {
        assert_eq!(site_user(" Files.Example.com "), "site:files.example.com");
    }
}
