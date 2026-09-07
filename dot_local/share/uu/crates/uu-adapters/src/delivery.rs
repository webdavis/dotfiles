//! Process and signed-post adapters for the run application's delivery ports.

use crate::alert::{Alerter, alert_argv};
use crate::config::Records;
use crate::record::record_state;
use crate::system::host;
mod alarm;
use pns_hermes::{PostOutcome, SignedPost, UreqSignedPost, delivered, outcome_line, sign};
use std::process::Command;
use std::time::Duration;
use uu_application::{
    AlarmKind, AlertOutcome, AlertTarget, RecordFailure, RecordOutcome, RunDelivery, RunRecord,
};
use uu_protocol::record_body;

/// How long one signed record POST may take. Nobody is waiting on the answer,
/// so this only bounds how long the job lingers on a gateway that stopped
/// listening; it matches the deadline pns gives its own unwatched posts.
const RECORD_DEADLINE: Duration = Duration::from_secs(10);

/// The pns engine, as a client: flags in, nothing read back.
pub struct PnsAlerter;

impl Alerter for PnsAlerter {
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
        match Command::new(binary).args(args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("`{binary}` answered {status}")),
            Err(error) => Err(format!("`{binary}` could not be run: {error}")),
        }
    }
}

pub struct EngineRunDelivery<'a, P, A> {
    pub post: P,
    pub alerter: A,
    pub records: Option<&'a Records>,
    pub engine: Option<&'a str>,
}

impl<'a> EngineRunDelivery<'a, UreqSignedPost, PnsAlerter> {
    pub fn new(records: Option<&'a Records>, engine: Option<&'a str>) -> Self {
        Self {
            post: UreqSignedPost,
            alerter: PnsAlerter,
            records,
            engine,
        }
    }
}

impl<P: SignedPost, A: Alerter> RunDelivery for EngineRunDelivery<'_, P, A> {
    fn alert(
        &self,
        kind: AlarmKind,
        host: &str,
        target: AlertTarget<'_>,
        summary: &str,
    ) -> AlertOutcome {
        let mut configured = false;
        let mut failures = Vec::new();
        if let Some(binary) = self.engine {
            configured = true;
            if let Err(why) = self
                .alerter
                .alert(binary, &alert_argv(host, target_name(target), summary))
            {
                failures.push(why);
            }
        }
        if let Some((records, url)) = self
            .records
            .and_then(|records| records.failure_webhook.as_deref().map(|url| (records, url)))
        {
            configured = true;
            if let Err(why) = self.alarm_post(records, url, kind, host, summary) {
                failures.push(why);
            }
        }
        if !failures.is_empty() {
            AlertOutcome::Failed(failures.join("; "))
        } else if configured {
            AlertOutcome::Delivered
        } else {
            AlertOutcome::NotConfigured
        }
    }

    fn record(&self, record: RunRecord<'_>) -> RecordOutcome {
        let Some(records) = self.records else {
            return RecordOutcome::NotConfigured;
        };
        let body = records_body(
            record.failures,
            record.deferred,
            record.pending,
            record.detail,
        );
        let Some(signature) = sign(&records.key, &body) else {
            return RecordOutcome::SigningFailed;
        };
        let outcome = self
            .post
            .post(&records.url, &body, &signature, Some(RECORD_DEADLINE));
        let description = outcome_line(outcome);
        if delivered(outcome) {
            return RecordOutcome::Delivered { description };
        }
        let cause = match outcome {
            PostOutcome::Status(status) => RecordFailure::Status(status),
            PostOutcome::NoResponse => RecordFailure::NoResponse,
            PostOutcome::NoStatus => RecordFailure::NoStatus,
        };
        RecordOutcome::Rejected {
            url: records.url.clone(),
            cause,
            description,
        }
    }
}

pub(crate) fn target_name(target: AlertTarget<'_>) -> &str {
    match target {
        AlertTarget::Run => uu_protocol::AGENT,
        AlertTarget::Lane(lane) => lane,
    }
}

fn records_body(failures: usize, deferred: usize, pending: usize, detail: &str) -> String {
    record_body(record_state(failures, deferred, pending), &host(), detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deferred_only_run_posts_a_body_stated_deferred_not_completed() {
        // record_state itself is pinned directly in record.rs; this instead
        // guards the CALL SITE here in `records_body`, where a mutant
        // passing `record_state(failures, 0)` would post every deferred-only
        // run as "completed" while leaving every `record_state` unit test
        // green.
        let body = records_body(0, 1, 0, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "deferred");
    }

    #[test]
    fn a_mixed_run_posts_a_body_stated_failed_not_deferred() {
        let body = records_body(1, 1, 0, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "failed");
    }
    #[test]
    fn a_pending_only_run_posts_a_body_stated_pending_not_completed() {
        let body = records_body(0, 0, 1, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "pending");
    }
}
