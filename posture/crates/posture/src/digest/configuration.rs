use posture_adapters::Notify;
use posture_domain::DigestLimits;
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
    pub notify: Notify,
    pub alarm: PathBuf,
    pub limits: DigestLimits,
}

impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<Self> {
        let home = variable("HOME")?;
        let notify = Notify::read(std::path::Path::new(&home));
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
        let defaults = DigestLimits::default();
        Some(Self {
            store,
            notify,
            alarm: "/usr/bin/osascript".into(),
            limits: DigestLimits {
                groups: numeric_or(variable("DIGEST_MAX_GROUPS"), defaults.groups),
                bullets_per_group: numeric_or(
                    variable("DIGEST_MAX_BULLETS_PER_GROUP"),
                    defaults.bullets_per_group,
                ),
                body_chars: numeric_or(variable("DIGEST_MAX_BODY_CHARS"), defaults.body_chars),
                field_chars: numeric_or(variable("DIGEST_MAX_FIELD_CHARS"), defaults.field_chars),
            },
        })
    }
}

fn numeric_or(value: Option<OsString>, default: usize) -> usize {
    let Some(value) = value.filter(|value| {
        !value.is_empty() && value.as_encoded_bytes().iter().all(u8::is_ascii_digit)
    }) else {
        return default;
    };
    // Saturating rather than rejecting: an absurd cap is harmless where it is
    // spent, so it reads as "uncapped" instead of failing the run.
    value
        .as_encoded_bytes()
        .iter()
        .fold(0_usize, |number, digit| {
            number
                .saturating_mul(10)
                .saturating_add(usize::from(digit - b'0'))
        })
}

#[cfg(test)]
mod tests;
