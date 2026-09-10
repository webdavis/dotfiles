use std::{ffi::OsString, path::PathBuf};

/// The default spool, which must stay byte-identical to the alerter's own
/// literal.
///
/// TWO INDEPENDENT LITERALS, one on each side, because the two processes never
/// speak. If they diverge, the digest watches a path nobody writes and is
/// silently empty forever, which is exactly the failure a daily silent message
/// cannot distinguish from a quiet day.
const DEFAULT_STORE: &str = "/.local/state/osquery-digest-spool/digest.ndjson";

pub(super) struct Configuration {
    pub store: PathBuf,
    pub pns: PathBuf,
    pub alarm: PathBuf,
}

impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<Self> {
        let home = variable("HOME")?;
        let mut pns = home.clone();
        pns.push("/.cargo/bin/pns");
        let store = match variable("OSQUERY_DIGEST_STORE") {
            // AN EMPTY OVERRIDE IS NOT AN OVERRIDE. It would name the process's
            // working directory, and the run would claim and rotate files there.
            Some(override_path) if !override_path.is_empty() => PathBuf::from(override_path),
            _ => {
                let mut default = home;
                default.push(DEFAULT_STORE);
                PathBuf::from(default)
            }
        };
        Some(Self {
            store,
            pns: pns.into(),
            alarm: "/usr/bin/osascript".into(),
        })
    }
}

#[cfg(test)]
mod tests;
