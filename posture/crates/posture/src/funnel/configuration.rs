use posture_adapters::parse_command_duration as duration;
use std::os::unix::fs::PermissionsExt;
use std::{ffi::OsString, path::PathBuf, time::Duration};

pub(super) struct Configuration {
    pub state: PathBuf,
    pub tailscale: PathBuf,
    pub pns: PathBuf,
    pub budget: Option<Duration>,
}
impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<Self> {
        let home = PathBuf::from(variable("HOME")?);
        let state = variable("OSQUERY_TAILSCALE_STATE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state/osquery-tailscale-funnel.json"));
        let tailscale = variable("OSQUERY_TAILSCALE_BIN")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::split_paths(&variable("PATH").unwrap_or_default())
                    .map(|directory| directory.join("tailscale"))
                    .find(|path| {
                        std::fs::metadata(path)
                            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    })
            })
            .unwrap_or_else(|| "/Applications/Tailscale.app/Contents/MacOS/Tailscale".into());
        let timeout = variable("OSQUERY_TAILSCALE_TIMEOUT").filter(|value| !value.is_empty());
        let budget = timeout
            .as_deref()
            .map_or(Some(Duration::from_secs(10)), |value| {
                duration(value.to_str()?)
            });
        Some(Self {
            state,
            tailscale,
            pns: home.join(".cargo/bin/pns"),
            budget,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_duration_that_cannot_be_added_to_a_deadline_is_refused() {
        assert!(duration("10000000000000000000").is_none());
    }
    #[test]
    fn finite_fractional_seconds_and_unit_suffixes_preserve_the_timeout() {
        for (literal, seconds) in [
            (" 0.5", 0.5),
            ("0x1p-1", 0.5),
            ("0x1d", 29.),
            ("0x1p-1m", 30.),
            ("0.02", 0.02),
            ("2s", 2.),
            ("0.5m", 30.),
            ("1h", 3600.),
            ("1d", 86400.),
        ] {
            assert_eq!(duration(literal), Some(Duration::from_secs_f64(seconds)));
        }
        for literal in [
            "0", "-1", "inf", "NaN", "bogus", "1e100", "0.5 ", "0.5ms", "0.5\0s",
        ] {
            assert!(duration(literal).is_none(), "{literal}");
        }
    }
    #[test]
    fn private_overrides_win_and_empty_values_use_the_existing_default_paths() {
        let vars = |name: &str| match name {
            "HOME" => Some(OsString::from("/private/fixture")),
            "OSQUERY_TAILSCALE_STATE" => Some(OsString::from("/private/other/state")),
            "OSQUERY_TAILSCALE_BIN" => Some(OsString::from("/private/other/tailscale")),
            "OSQUERY_TAILSCALE_TIMEOUT" => Some(OsString::from("0.02")),
            _ => None,
        };
        let config = Configuration::read(vars).unwrap();
        assert_eq!(config.state, PathBuf::from("/private/other/state"));
        assert_eq!(config.tailscale, PathBuf::from("/private/other/tailscale"));
        assert_eq!(config.pns, PathBuf::from("/private/fixture/.cargo/bin/pns"));
        assert_eq!(config.budget, Some(Duration::from_millis(20)));
        let config = Configuration::read(|name| {
            if name == "HOME" {
                Some("/private/fixture".into())
            } else {
                Some("".into())
            }
        })
        .unwrap();
        assert_eq!(
            config.state,
            PathBuf::from("/private/fixture/.local/state/osquery-tailscale-funnel.json")
        );
        assert_eq!(
            config.tailscale,
            PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale")
        );
        assert_eq!(config.budget, Some(Duration::from_secs(10)));
    }
}
