use posture_domain::{ReadinessRefusal, SshReadiness, ssh_verify_deadline};
use std::{ffi::OsString, path::PathBuf, time::Duration};

pub(super) struct Configuration {
    pub dropins: PathBuf,
    pub main: PathBuf,
    pub sshd: PathBuf,
    pub sudo: Option<PathBuf>,
    pub launchctl: PathBuf,
    pub keyscan: PathBuf,
    pub deadline: Duration,
    pub grace: Duration,
    pub attempts: OsString,
    pub interval: OsString,
    pub probe_timeout: OsString,
    pub allow_missing: bool,
}
impl Configuration {
    pub fn read(get: impl Fn(&str) -> Option<OsString>) -> Self {
        let path = |key, default| {
            PathBuf::from(
                get(key)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| OsString::from(default)),
            )
        };
        let value = |key, default| get(key).unwrap_or_else(|| OsString::from(default));
        let sudo = value("SSH_HARDENING_SUDO", "sudo");
        let allow_missing = value("SSH_HARDENING_ALLOW_MISSING_SSHD", "")
            .to_string_lossy()
            .to_ascii_lowercase();
        Self {
            dropins: path("SSHD_CONFIG_D", "/etc/ssh/sshd_config.d"),
            main: path("SSHD_MAIN_CONFIG", "/etc/ssh/sshd_config"),
            sshd: path("SSHD_BIN", "/usr/sbin/sshd"),
            sudo: (!sudo.is_empty()).then(|| PathBuf::from(sudo)),
            launchctl: path("LAUNCHCTL_BIN", "/bin/launchctl"),
            keyscan: path("KEYSCAN_BIN", "/usr/bin/ssh-keyscan"),
            deadline: ssh_verify_deadline(
                &value("SSH_HARDENING_VERIFY_DEADLINE", "").to_string_lossy(),
            ),
            grace: Duration::from_secs(2),
            attempts: value("SSH_HARDENING_READY_ATTEMPTS", "30"),
            interval: value("SSH_HARDENING_READY_INTERVAL", "1"),
            probe_timeout: value("SSH_HARDENING_PROBE_TIMEOUT", "5"),
            allow_missing: matches!(allow_missing.as_str(), "1" | "true" | "yes" | "on"),
        }
    }
    pub fn readiness(&self) -> Result<SshReadiness, ReadinessRefusal> {
        SshReadiness::parse(
            &self.attempts.to_string_lossy(),
            &self.interval.to_string_lossy(),
            &self.probe_timeout.to_string_lossy(),
        )
    }
}
