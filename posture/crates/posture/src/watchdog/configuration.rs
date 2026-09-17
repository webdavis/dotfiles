use posture_adapters::Notify;
use posture_domain::{AgentLabels, AuditBounds, ManifestAuthority};
use std::{ffi::OsString, path::PathBuf, time::Duration};

pub(super) struct Configuration {
    pub snapshots: PathBuf,
    pub state: PathBuf,
    pub legacy_queue: PathBuf,
    pub pns_ledger: PathBuf,
    pub pipeline: PathBuf,
    pub managed_bin: PathBuf,
    pub authority: [ManifestAuthority; 2],
    pub pns: PathBuf,
    pub notify: Notify,
    /// The launchd label of each of posture's own jobs, read from its config.
    pub agents: AgentLabels,
    pub alarm: PathBuf,
    pub gateway: String,
    pub route_timeout: Duration,
    pub maximum_age: u64,
    pub bounds: AuditBounds,
}
impl Configuration {
    pub fn read(get: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let value = |key| get(key).filter(|v| !v.is_empty());
        let home = PathBuf::from(value("HOME")?);
        let path = |key, fallback| value(key).map(PathBuf::from).unwrap_or(fallback);
        let text = |key| {
            value(key)
                .and_then(|v| v.into_string().ok())
                .unwrap_or_default()
        };
        let manifest = |key, fallback| {
            (
                path(key, PathBuf::from(fallback)),
                if value(key).is_some() {
                    ManifestAuthority::ExplicitOverride
                } else {
                    ManifestAuthority::Protected
                },
            )
        };
        let (pipeline, pipeline_authority) = manifest(
            "OSQUERY_PIPELINE_MANIFEST",
            "/var/osquery/pipeline-known-good.sha256",
        );
        let (managed_bin, bin_authority) = manifest(
            "OSQUERY_MANAGED_BIN_MANIFEST",
            "/var/osquery/managed-bin-known-good.sha256",
        );
        let age = text("OSQUERY_CANARY_MAX_AGE");
        let maximum_age = if !age.is_empty() && age.bytes().all(|b| b.is_ascii_digit()) {
            age.parse().unwrap_or(1800)
        } else {
            1800
        };
        let timeout = text("OSQUERY_WATCHDOG_ROUTE_TIMEOUT")
            .parse::<f64>()
            .ok()
            .and_then(|n| Duration::try_from_secs_f64(n).ok())
            .filter(|duration| !duration.is_zero())
            .unwrap_or(Duration::from_secs(3));
        Some(Self {
            snapshots: path(
                "OSQUERY_SNAPSHOTS_LOG",
                home.join(".local/log/osquery/osqueryd.snapshots.log"),
            ),
            state: path(
                "OSQUERY_WATCHDOG_STATE",
                home.join(".local/state/osquery-watchdog-state.json"),
            ),
            legacy_queue: path(
                "OSQUERY_UNDELIVERED_ALERTS_DB",
                home.join(".local/state/osquery-undelivered-alerts.sqlite3"),
            ),
            pns_ledger: path("PNS_STATE_DIR", home.join(".local/state/pns")).join("pns.db"),
            pipeline,
            managed_bin,
            authority: [pipeline_authority, bin_authority],
            pns: home.join(".cargo/bin/pns"),
            notify: Notify::read(&home),
            agents: posture_adapters::agent_labels(&home),
            alarm: PathBuf::from("/usr/bin/osascript"),
            gateway: value("OSQUERY_HERMES_PRIORITY_URL")
                .and_then(|v| v.into_string().ok())
                .unwrap_or_else(|| "http://127.0.0.1:8644/webhooks/priority".into()),
            route_timeout: timeout,
            maximum_age,
            bounds: AuditBounds::from_values(
                &text("OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES"),
                &text("OSQUERY_PIPELINE_AUDIT_MAX_BYTES"),
                &text("OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS"),
            ),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_binary_and_legacy_state_paths_are_kept_distinct_from_gateway_health() {
        let c = Configuration::read(|key| (key == "HOME").then(|| OsString::from("/private/home")))
            .unwrap();
        assert_eq!(c.pns, PathBuf::from("/private/home/.cargo/bin/pns"));
        assert_eq!(
            c.state,
            PathBuf::from("/private/home/.local/state/osquery-watchdog-state.json")
        );
        assert_eq!(
            c.pns_ledger,
            PathBuf::from("/private/home/.local/state/pns/pns.db")
        );
        assert_eq!(c.authority, [ManifestAuthority::Protected; 2]);
        assert_eq!(c.gateway, "http://127.0.0.1:8644/webhooks/priority");
    }
    #[test]
    fn existing_probe_overrides_are_scoped_and_cannot_remove_bounds() {
        let c = Configuration::read(|key| match key {
            "HOME" => Some("/private/home".into()),
            "PNS_STATE_DIR" => Some("/private/pns-state".into()),
            "OSQUERY_PIPELINE_MANIFEST" => Some("/private/pipeline".into()),
            "OSQUERY_WATCHDOG_STATE" => Some("/private/state".into()),
            "OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS" => Some("0".into()),
            "OSQUERY_CANARY_MAX_AGE" => Some("bad".into()),
            "OSQUERY_WATCHDOG_ROUTE_TIMEOUT" => Some("0".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.state, PathBuf::from("/private/state"));
        assert_eq!(c.pns_ledger, PathBuf::from("/private/pns-state/pns.db"));
        assert_eq!(
            c.authority,
            [
                ManifestAuthority::ExplicitOverride,
                ManifestAuthority::Protected
            ]
        );
        assert_eq!(c.bounds.seconds, 0);
        assert_eq!(c.maximum_age, 1800);
        assert_eq!(c.route_timeout, Duration::from_secs(3));
    }
    #[test]
    fn positive_operator_route_deadlines_are_preserved() {
        let c = Configuration::read(|key| match key {
            "HOME" => Some("/private/home".into()),
            "OSQUERY_WATCHDOG_ROUTE_TIMEOUT" => Some("900.5".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.route_timeout, Duration::from_millis(900500));
    }
}
