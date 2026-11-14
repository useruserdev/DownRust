// SPDX-License-Identifier: GPL-3.0-or-later
//! SFTP transfers over an SSH session with a password login. The server host key is accepted and
//! its fingerprint is logged.
use std::io::SeekFrom;
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, AuthResult, Handler};
use russh::keys::{HashAlg, PublicKeyOrCertificate};
use russh::Disconnect;
use russh_sftp::client::SftpSession;
use tokio::io::AsyncSeekExt;

use super::address::{Address, Login};
use super::pump::copy_to_part;
use super::{bounded, fatal, prepare, retry, Step};
use crate::engine::task::TaskContext;
use crate::model::DownloadRecord;

/// SSH client handler. It accepts the server key and logs its fingerprint.
struct HostKeys {
    host: String,
}

impl Handler for HostKeys {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => tracing::info!(
                host = %self.host,
                fingerprint = %key.fingerprint(HashAlg::Sha256),
                "SFTP server host key"
            ),
            PublicKeyOrCertificate::Certificate(_) => {
                tracing::info!(host = %self.host, "SFTP server host certificate");
            }
        }
        Ok(true)
    }
}

/// Downloads the file. Returns its size when the transfer is complete.
pub(super) async fn download(
    ctx: &TaskContext,
    prev: &DownloadRecord,
    address: &Address,
    login: &Login,
) -> Step<u64> {
    let connect_limit = Duration::from_secs(ctx.settings.connect_timeout_secs);
    let limit = Duration::from_secs(ctx.settings.read_timeout_secs);
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(limit),
        ..client::Config::default()
    });
    let handler = HostKeys {
        host: address.host.clone(),
    };
    let mut session = bounded(
        ctx,
        connect_limit,
        "connecting",
        client::connect(config, (address.host.as_str(), address.port), handler),
        |err| retry(err.to_string(), false),
    )
    .await?;

    let user = login.user.clone();
    let password = login.password.clone().unwrap_or_default();
    let auth = bounded(
        ctx,
        limit,
        "the login",
        session.authenticate_password(user.clone(), password),
        |err| retry(err.to_string(), false),
    )
    .await?;
    if !matches!(auth, AuthResult::Success) {
        return Err(fatal(format!(
            "The SFTP server rejected the login for {user}."
        )));
    }

    let channel = bounded(
        ctx,
        limit,
        "the SSH channel",
        session.channel_open_session(),
        |err| retry(err.to_string(), false),
    )
    .await?;
    bounded(
        ctx,
        limit,
        "the SFTP subsystem",
        channel.request_subsystem(true, "sftp"),
        |err| retry(err.to_string(), false),
    )
    .await?;
    let sftp = bounded(
        ctx,
        limit,
        "the SFTP session",
        SftpSession::new(channel.into_stream()),
        |err| retry(err.to_string(), false),
    )
    .await?;

    let path = address.path.clone();
    let attrs = bounded(
        ctx,
        limit,
        "the file information",
        sftp.metadata(path.clone()),
        |err| fatal(format!("The SFTP server refused {path}: {err}")),
    )
    .await?;
    let size = attrs
        .size
        .ok_or_else(|| fatal("The SFTP server did not report the file size."))?;

    let (file, offset) = prepare(ctx, prev, size)?;
    if offset < size {
        let mut remote = bounded(
            ctx,
            limit,
            "opening the file",
            sftp.open(path.clone()),
            |err| fatal(format!("cannot open {path}: {err}")),
        )
        .await?;
        remote
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|err| retry(err.to_string(), false))?;
        let end = copy_to_part(ctx, &file, offset, &mut remote).await?;
        if end < size {
            return Err(retry(
                format!("the connection closed at {end} of {size} bytes"),
                end > offset,
            ));
        }
    }
    if let Err(err) = session
        .disconnect(Disconnect::ByApplication, "", "en")
        .await
    {
        tracing::debug!(id = ctx.id, error = %err, "SFTP disconnect failed");
    }
    Ok(size)
}
