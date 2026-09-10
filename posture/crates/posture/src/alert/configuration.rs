use std::{ffi::OsString, path::PathBuf};

/// The results log the alerter reads, relative to `$HOME`.
const DEFAULT_LOG: &str = "/.local/log/osquery/osqueryd.results.log";
/// Where it records how far it got.
const DEFAULT_CURSOR: &str = "/.local/state/osquery-results-offset";
/// The launchd page-allowlist it consults.
const DEFAULT_ALLOWLIST: &str = "/.config/osquery/page-launchd-allowlist.txt";
/// The digest spool it appends to.
///
/// TWO INDEPENDENT LITERALS, one on each side, because the alerter and the
/// daily digest never speak. If they diverge, the digest watches a path nobody
/// writes and is silently empty forever, which is exactly the failure a daily
/// silent message cannot distinguish from a quiet day.
const DEFAULT_SPOOL: &str = "/.local/state/osquery-digest-spool/digest.ndjson";

/// The root-owned manifests the file-integrity verdict consults.
const DEFAULT_PIPELINE_MANIFEST: &str = "/var/osquery/pipeline-known-good.sha256";
const DEFAULT_MANAGED_BIN_MANIFEST: &str = "/var/osquery/managed-bin-known-good.sha256";

pub(super) struct Configuration {
    pub home: String,
    pub pipeline_manifest: PathBuf,
    pub managed_bin_manifest: PathBuf,
    pub log: PathBuf,
    pub cursor: PathBuf,
    pub allowlist: PathBuf,
    pub spool: PathBuf,
    pub pns: PathBuf,
    pub alarm: PathBuf,
}

impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<Self> {
        let home = variable("HOME")?;
        // EVERY OVERRIDE IS EMPTY-CHECKED, because an empty one names the
        // process's working directory. For the cursor that would put the
        // alerter's state wherever launchd happened to start it, and for the
        // log it would read a directory as a file every tick.
        let mut resolve = |name: &str, default: &str, under_home: bool| -> PathBuf {
            match variable(name) {
                Some(value) if !value.is_empty() => PathBuf::from(value),
                _ if under_home => {
                    let mut path = home.clone();
                    path.push(default);
                    PathBuf::from(path)
                }
                // THE MANIFESTS ARE ABSOLUTE, not under HOME. They are
                // root-owned in /var/osquery precisely so the account being
                // watched cannot rewrite them, which is the whole reason a
                // manifest is worth consulting.
                _ => PathBuf::from(default),
            }
        };
        let log = resolve("OSQUERY_RESULTS_LOG", DEFAULT_LOG, true);
        let cursor = resolve("OSQUERY_RESULTS_OFFSET", DEFAULT_CURSOR, true);
        let allowlist = resolve("OSQUERY_LAUNCHD_ALLOWLIST", DEFAULT_ALLOWLIST, true);
        let spool = resolve("OSQUERY_DIGEST_STORE", DEFAULT_SPOOL, true);
        let pipeline_manifest = resolve(
            "OSQUERY_PIPELINE_MANIFEST",
            DEFAULT_PIPELINE_MANIFEST,
            false,
        );
        let managed_bin_manifest = resolve(
            "OSQUERY_MANAGED_BIN_MANIFEST",
            DEFAULT_MANAGED_BIN_MANIFEST,
            false,
        );
        let mut pns = home.clone();
        pns.push("/.cargo/bin/pns");
        // THE MANIFESTS ARE ABSOLUTE, not under HOME. They are root-owned in
        // /var/osquery precisely so the account being watched cannot rewrite
        // them, which is the whole reason a manifest is worth consulting.
        Some(Self {
            home: home.to_string_lossy().into_owned(),
            log,
            cursor,
            allowlist,
            spool,
            pipeline_manifest,
            managed_bin_manifest,
            pns: pns.into(),
            alarm: "/usr/bin/osascript".into(),
        })
    }
}

#[cfg(test)]
mod tests;
