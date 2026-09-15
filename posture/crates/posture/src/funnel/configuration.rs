use posture_adapters::{Delivery, is_executable, parse_command_duration as duration};
use std::{ffi::OsString, path::PathBuf, time::Duration};

pub(super) struct Configuration {
    pub state: PathBuf,
    pub tailscale: PathBuf,
    pub delivery: Delivery,
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
                    .find(|path| is_executable(path))
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
            delivery: Delivery::read(&home),
            budget,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use posture_adapters::COMMAND_DURATION_CEILING;
    #[test]
    fn a_disabled_or_unrepresentable_timeout_reads_as_the_ceiling_not_as_a_failure() {
        // timeout(1) runs unlimited for all five, so none of them may become a
        // read failure that pages a gap claiming a command that never ran exited.
        for literal in ["0", "0m", "inf", "1e100", "10000000000000000000"] {
            assert_eq!(
                duration(literal),
                Some(COMMAND_DURATION_CEILING),
                "{literal}"
            );
        }
        assert_eq!(
            Configuration::read(|name| match name {
                "HOME" => Some(OsString::from("/private/fixture")),
                "OSQUERY_TAILSCALE_TIMEOUT" => Some(OsString::from("0")),
                _ => None,
            })
            .unwrap()
            .budget,
            Some(COMMAND_DURATION_CEILING)
        );
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
        // timeout(1) calls each of these an invalid time interval and exits 125,
        // which is the exit code a refusal here goes on to report.
        for literal in ["-1", "NaN", "bogus", "0.5 ", "0.5ms", "0.5\0s"] {
            assert!(duration(literal).is_none(), "{literal}");
        }
    }
    #[test]
    fn a_path_entry_this_identity_cannot_execute_is_not_the_tailscale_binary() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let root = std::env::temp_dir().join(format!("posture-funnel-path-{}", std::process::id()));
        // A pid comes round again, and a failed run leaves the file below
        // unwritable, so the tree is cleared rather than reused.
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let candidate = root.join("tailscale");
        std::fs::write(&candidate, "#!/bin/sh\n").unwrap();
        let vars = |name: &str| match name {
            "HOME" => Some(OsString::from("/private/fixture")),
            "PATH" => Some(OsString::from(root.as_os_str())),
            _ => None,
        };
        // Execute for group and other but never for this identity, the way a
        // root-owned 0700 binary reads to an unprivileged poller. Root is
        // exempt: faccessat grants X_OK on any execute bit at all.
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o011)).unwrap();
        if std::fs::metadata(&candidate).unwrap().uid() != 0 {
            assert_eq!(
                Configuration::read(vars).unwrap().tailscale,
                PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale")
            );
        }
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(Configuration::read(vars).unwrap().tailscale, candidate);
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
