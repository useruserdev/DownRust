// SPDX-License-Identifier: GPL-3.0-or-later
//! Command line entry point. Runs the test server until the process is stopped.

use downrust_testserver::{file_url, Error, FileOpts, Result, TestServer};

const DEFAULT_PORT: u16 = 8787;

/// Reads `--port <n>` or `--port=<n>`. Defaults to 8787.
fn parse_port(mut args: impl Iterator<Item = String>) -> Result<u16> {
    let mut port = DEFAULT_PORT;
    while let Some(arg) = args.next() {
        let value = if arg == "--port" {
            args.next()
                .ok_or_else(|| Error::Usage("--port needs a value".to_owned()))?
        } else if let Some(value) = arg.strip_prefix("--port=") {
            value.to_owned()
        } else {
            return Err(Error::Usage(format!(
                "unknown argument `{arg}`; usage: downrust-testserver [--port <n>]"
            )));
        };
        port = value
            .parse()
            .map_err(|_| Error::Usage(format!("invalid port `{value}`")))?;
    }
    Ok(port)
}

fn main() -> Result<()> {
    let port = parse_port(std::env::args().skip(1))?;
    let server = TestServer::start_on(port)?;
    let base = format!("http://{}", server.addr);
    println!("DownRust test server listening on {base}");
    println!(
        "  rate-limited 200 MB, 1 MiB/s: {base}{}",
        file_url(
            200 * 1024 * 1024,
            &FileOpts {
                rate: 1024 * 1024,
                ..FileOpts::default()
            }
        )
    );
    println!(
        "  no range support:             {base}{}",
        file_url(
            1000,
            &FileOpts {
                ranges: false,
                ..FileOpts::default()
            }
        )
    );
    println!(
        "  unknown length:               {base}{}",
        file_url(
            1000,
            &FileOpts {
                length: false,
                ..FileOpts::default()
            }
        )
    );
    println!(
        "  named download:               {base}{}",
        file_url(
            1000,
            &FileOpts {
                name: Some("report.pdf".to_owned()),
                ..FileOpts::default()
            }
        )
    );
    println!("Press Ctrl+C to stop.");
    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_argument_forms() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(parse_port(args(&[]).into_iter()).ok(), Some(DEFAULT_PORT));
        assert_eq!(
            parse_port(args(&["--port", "9000"]).into_iter()).ok(),
            Some(9000)
        );
        assert_eq!(
            parse_port(args(&["--port=9001"]).into_iter()).ok(),
            Some(9001)
        );
        assert!(parse_port(args(&["--port"]).into_iter()).is_err());
        assert!(parse_port(args(&["--port", "abc"]).into_iter()).is_err());
        assert!(parse_port(args(&["--bogus"]).into_iter()).is_err());
    }
}
