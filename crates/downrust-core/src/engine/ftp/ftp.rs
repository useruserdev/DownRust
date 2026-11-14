// SPDX-License-Identifier: GPL-3.0-or-later
//! FTP and FTPS transfers: passive mode, binary type, resume with REST, explicit TLS for FTPS.
use std::sync::Arc;
use std::time::Duration;

use suppaftp::rustls::{ClientConfig, RootCertStore};
use suppaftp::tokio::{AsyncFtpStream, AsyncRustlsConnector, ImplAsyncFtpStream, TokioTlsStream};
use suppaftp::tokio_rustls::TlsConnector;
use suppaftp::types::{FileType, FtpError};
use tokio::net::TcpStream;

use super::address::{Address, Login, Protocol};
use super::pump::copy_to_part;
use super::{bounded, fatal, prepare, retry, Failure, Step};
use crate::engine::task::TaskContext;
use crate::model::DownloadRecord;

/// Downloads the file. Returns its size when the transfer is complete.
pub(super) async fn download(
    ctx: &TaskContext,
    prev: &DownloadRecord,
    address: &Address,
    login: &Login,
) -> Step<u64> {
    let connect_limit = Duration::from_secs(ctx.settings.connect_timeout_secs);
    let tcp = bounded(
        ctx,
        connect_limit,
        "connecting",
        TcpStream::connect((address.host.as_str(), address.port)),
        |err| retry(err.to_string(), false),
    )
    .await?;
    match address.protocol {
        Protocol::Ftps => {
            let plain = bounded(
                ctx,
                connect_limit,
                "the FTP greeting",
                ImplAsyncFtpStream::connect_with_stream(tcp),
                ftp_failure,
            )
            .await?;
            let connector = tls_connector()?;
            let secure = bounded(
                ctx,
                connect_limit,
                "the TLS handshake",
                plain.into_secure(connector, &address.host),
                ftp_failure,
            )
            .await?;
            session(ctx, prev, address, login, secure).await
        }
        Protocol::Ftp | Protocol::Sftp => {
            let plain: AsyncFtpStream = bounded(
                ctx,
                connect_limit,
                "the FTP greeting",
                AsyncFtpStream::connect_with_stream(tcp),
                ftp_failure,
            )
            .await?;
            session(ctx, prev, address, login, plain).await
        }
    }
}

/// Logs in and transfers the file over an open control connection.
async fn session<T>(
    ctx: &TaskContext,
    prev: &DownloadRecord,
    address: &Address,
    login: &Login,
    mut ftp: ImplAsyncFtpStream<T>,
) -> Step<u64>
where
    T: TokioTlsStream + Send,
{
    let limit = Duration::from_secs(ctx.settings.read_timeout_secs);
    let user = login.user.clone();
    let password = login.password.clone().unwrap_or_default();
    bounded(
        ctx,
        limit,
        "the login",
        ftp.login(&user, &password),
        |err| login_failure(err, &user),
    )
    .await?;
    bounded(
        ctx,
        limit,
        "the transfer type",
        ftp.transfer_type(FileType::Binary),
        ftp_failure,
    )
    .await?;
    let path = address.path.clone();
    let size = bounded(ctx, limit, "the size request", ftp.size(&path), |err| {
        size_failure(err, &path)
    })
    .await? as u64;
    let (file, offset) = prepare(ctx, prev, size)?;
    if offset < size {
        bounded(
            ctx,
            limit,
            "the resume offset",
            ftp.resume_transfer(offset as usize),
            ftp_failure,
        )
        .await?;
        let mut data = bounded(
            ctx,
            limit,
            "the data connection",
            ftp.retr_as_stream(&path),
            ftp_failure,
        )
        .await?;
        let end = copy_to_part(ctx, &file, offset, &mut data).await?;
        if end < size {
            return Err(retry(
                format!("the connection closed at {end} of {size} bytes"),
                end > offset,
            ));
        }
        if let Err(err) = data.finish().await {
            tracing::warn!(id = ctx.id, error = %err, "FTP transfer end reply was not read");
        }
    }
    if let Err(err) = ftp.quit().await {
        tracing::debug!(id = ctx.id, error = %err, "FTP quit failed");
    }
    Ok(size)
}

/// TLS settings for FTPS: the system certificate roots and the ring crypto provider.
fn tls_connector() -> Step<AsyncRustlsConnector> {
    let mut roots = RootCertStore::empty();
    let loaded = rustls_native_certs::load_native_certs();
    for err in loaded.errors {
        tracing::warn!("a system certificate could not be read: {err}");
    }
    for cert in loaded.certs {
        let _ = roots.add(cert);
    }
    let config = ClientConfig::builder_with_provider(Arc::new(
        suppaftp::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(fatal)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(AsyncRustlsConnector::from(TlsConnector::from(Arc::new(
        config,
    ))))
}

/// A refused login stops the download. A lost connection is tried again.
fn login_failure(err: FtpError, user: &str) -> Failure {
    match err {
        FtpError::UnexpectedResponse(_) => {
            Failure::Fatal(format!("The FTP server rejected the login for {user}."))
        }
        other => ftp_failure(other),
    }
}

/// A missing or refused file stops the download. A lost connection is tried again.
fn size_failure(err: FtpError, path: &str) -> Failure {
    match err {
        FtpError::ConnectionError(_) => ftp_failure(err),
        other => Failure::Fatal(format!("The FTP server refused {path}: {other}")),
    }
}

/// A lost connection is tried again. Any other error stops the download.
fn ftp_failure(err: FtpError) -> Failure {
    match err {
        FtpError::ConnectionError(io) => retry(io.to_string(), false),
        other => fatal(other),
    }
}
