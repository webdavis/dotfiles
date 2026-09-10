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

pub(super) struct Configuration {
    pub home: String,
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
        let mut resolve = |name: &str, default: &str| -> PathBuf {
            match variable(name) {
                Some(value) if !value.is_empty() => PathBuf::from(value),
                _ => {
                    let mut path = home.clone();
                    path.push(default);
                    PathBuf::from(path)
                }
            }
        };
        let mut pns = home.clone();
        pns.push("/.cargo/bin/pns");
        Some(Self {
            home: home.to_string_lossy().into_owned(),
            log: resolve("OSQUERY_RESULTS_LOG", DEFAULT_LOG),
            cursor: resolve("OSQUERY_RESULTS_OFFSET", DEFAULT_CURSOR),
            allowlist: resolve("OSQUERY_LAUNCHD_ALLOWLIST", DEFAULT_ALLOWLIST),
            spool: resolve("OSQUERY_DIGEST_STORE", DEFAULT_SPOOL),
            pns: pns.into(),
            alarm: "/usr/bin/osascript".into(),
        })
    }
}

#[cfg(test)]
mod tests;
