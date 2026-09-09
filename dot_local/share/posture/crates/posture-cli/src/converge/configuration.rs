use posture_domain::RestartBounds;
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ConfigurationRefusal {
    UnexpectedOverride(&'static str),
    MissingSandboxPath(&'static str),
}

#[derive(Debug)]
pub(super) struct Configuration {
    pub desired: PathBuf,
    pub target: PathBuf,
    pub sudo: PathBuf,
    pub osqueryctl: Option<PathBuf>,
    pub search_path: OsString,
    pub log_directory: PathBuf,
    pub bounds: RestartBounds,
}

impl Configuration {
    pub(super) fn read(
        mut variable: impl FnMut(&str) -> Option<OsString>,
    ) -> Result<Self, Vec<ConfigurationRefusal>> {
        let names = [
            "OSQUERY_CONVERGE_DESIRED_DIR",
            "OSQUERY_CONVERGE_TARGET_DIR",
            "OSQUERY_CONVERGE_SUDO",
            "OSQUERY_CONVERGE_OSQUERYCTL",
        ];
        let overrides = names.map(&mut variable);
        let test_seam = variable("OSQUERY_CONVERGE_TEST_SEAM").is_some_and(|value| value == "1");
        if !test_seam {
            let refused: Vec<_> = names
                .into_iter()
                .zip(&overrides)
                .filter(|(_, value)| value.is_some())
                .map(|(name, _)| ConfigurationRefusal::UnexpectedOverride(name))
                .collect();
            if !refused.is_empty() {
                return Err(refused);
            }
        } else {
            for index in [1, 2] {
                if overrides[index]
                    .as_ref()
                    .is_none_or(|value| value.is_empty())
                {
                    return Err(vec![ConfigurationRefusal::MissingSandboxPath(names[index])]);
                }
            }
        }
        let [desired, target, sudo, osqueryctl] =
            overrides.map(|value| value.filter(|value| !value.is_empty()));
        let home = PathBuf::from(variable("HOME").unwrap_or_default());
        let deadline = variable("OSQUERY_CONVERGE_RESTART_DEADLINE");
        let settle = variable("OSQUERY_CONVERGE_SETTLE_SECONDS");
        Ok(Self {
            desired: desired
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/libexec/osquery/osquery-converge/desired")),
            target: target
                .map(PathBuf::from)
                .unwrap_or_else(|| "/var/osquery".into()),
            sudo: sudo
                .map(PathBuf::from)
                .unwrap_or_else(|| "/usr/bin/sudo".into()),
            osqueryctl: osqueryctl.map(PathBuf::from),
            search_path: variable("PATH").unwrap_or_default(),
            log_directory: variable("OSQUERY_CONVERGE_LOG_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/log/osquery")),
            bounds: RestartBounds::parse(
                deadline.as_deref().and_then(|value| value.to_str()),
                settle.as_deref().and_then(|value| value.to_str()),
            ),
        })
    }
}

#[cfg(test)]
mod tests;
