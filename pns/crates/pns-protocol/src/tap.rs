use crate::envelope::{Rejected, encode};
use crate::identifiers::SchemaId;
use serde::Serialize;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapOperation {
    Tap,
    Info,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapWriteStatus {
    NotRequested,
    Recorded,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct TapMarker {
    pub path: String,
    pub source: String,
    pub config_file: String,
    pub exists: Option<bool>,
    pub mtime_epoch_secs: Option<u64>,
    /// The same instant as `mtime_epoch_secs`, RFC 3339 in UTC, for a reader
    /// that shows the tap to a person rather than computing an age from it.
    pub touched_at: Option<String>,
    pub age_secs: Option<u64>,
    pub fresh: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TapInstallStep {
    pub title: String,
    pub blurb: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TapInstall {
    pub binary: String,
    pub host: String,
    pub user: String,
    pub authorized_key_line: String,
    pub shortcut_url: Option<String>,
    pub verified_ios: Option<String>,
    pub steps: Vec<TapInstallStep>,
    pub undo: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TapError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct TapResult {
    pub operation: TapOperation,
    pub ok: bool,
    pub write_status: TapWriteStatus,
    pub marker: Option<TapMarker>,
    pub surface: Option<String>,
    pub message: String,
    pub install: Option<TapInstall>,
    pub error: Option<TapError>,
}

impl TapResult {
    pub fn new(operation: TapOperation) -> Self {
        Self {
            operation,
            ok: true,
            write_status: TapWriteStatus::NotRequested,
            marker: None,
            surface: None,
            message: String::new(),
            install: None,
            error: None,
        }
    }

    pub fn fail(&mut self, code: &str, message: &str) {
        self.ok = false;
        self.message = message.to_string();
        self.error = Some(TapError {
            code: code.to_string(),
            message: message.to_string(),
        });
    }

    pub fn encode(&self) -> Result<String, Rejected> {
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'static str,
            #[serde(flatten)]
            result: &'a TapResult,
        }
        encode(
            &Wire {
                schema: "pns.tap/1",
                result: self,
            },
            &SchemaId {
                name: "pns.tap".into(),
                major: 1,
            },
        )
    }
}
