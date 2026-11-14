// SPDX-License-Identifier: GPL-3.0-or-later
//! Virus scan that runs after a download completes.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Arguments of the built-in Windows Defender command line scanner.
pub const DEFAULT_SCAN_ARGS: &str = r#"-Scan -ScanType 3 -File "{file}" -DisableRemediation"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanSettings {
    pub enabled: bool,
    /// Program to run. Empty selects the Windows Defender command line scanner.
    pub program: String,
    /// Arguments. `{file}` is replaced with the path of the finished file.
    pub args: String,
}

impl Default for ScanSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            program: String::new(),
            args: DEFAULT_SCAN_ARGS.to_owned(),
        }
    }
}

impl ScanSettings {
    /// Trims the program text.
    pub fn sanitized(mut self) -> Self {
        self.program = self.program.trim().to_owned();
        self
    }

    /// The program to run: the configured one, or the Windows Defender scanner when none is set.
    pub fn resolved_program(&self) -> String {
        if !self.program.is_empty() {
            return self.program.clone();
        }
        let base = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_owned());
        PathBuf::from(base)
            .join("Windows Defender")
            .join("MpCmdRun.exe")
            .to_string_lossy()
            .into_owned()
    }
}
