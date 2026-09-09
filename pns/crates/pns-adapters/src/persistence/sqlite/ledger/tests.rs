use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn state() -> std::path::PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("the canonical temp directory")
        .join(format!(
            "pns-ledger-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
}
use pns_domain::{Event, routing::ReportMode};
fn submission() -> LedgerSubmission {
    LedgerSubmission {
        producer_request: None,
        identity: SubmissionIdentity {
            producer: "osquery".into(),
            request_id: "original-id".into(),
        },
        event: Event {
            agent: "agent".into(),
            state: "blocked".into(),
            project: "project".into(),
            branch: "branch".into(),
            detail: "detail\nsecond".into(),
            title: "title".into(),
            message: "message".into(),
            preview: "preview".into(),
            pane: "pane".into(),
        },
        legs: vec![
            LedgerLeg {
                destination: "phone-primary".into(),
                route: "urgent".into(),
                mode: ReportMode::ReportOutcome,
                decorative: false,
            },
            LedgerLeg {
                destination: "lights-desk".into(),
                route: "quiet".into(),
                mode: ReportMode::Silent,
                decorative: true,
            },
        ],
    }
}
fn lease(now: u64, until: u64) -> LeaseWindow {
    LeaseWindow { now, until }
}
fn created(store: &SqliteStore, submission: &LedgerSubmission) -> Vec<ClaimedLeg<DeliveryClaim>> {
    match store.prepare(submission, lease(10, 20)).unwrap() {
        PreparedSubmission::Created { legs, .. } => legs,
        other => panic!("expected created, got {other:?}"),
    }
}
fn acknowledged() -> LedgerCompletion {
    LedgerCompletion::Acknowledged {
        detail: "gateway accepted".into(),
    }
}
fn retry(at: u64) -> LedgerCompletion {
    LedgerCompletion::Retry {
        outcome: UnconfirmedDelivery::Failed,
        detail: "transport refused".into(),
        retry_at: at,
    }
}
/// A gateway that answers one fixed status to everything. Shared, because the
/// retry schedule, the permanent-versus-temporary split and the schema all
/// need a hermes leg whose answer the test chooses.
struct Reply(u16);
impl pns_hermes::SignedPost for Reply {
    fn post(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<std::time::Duration>,
    ) -> pns_hermes::PostOutcome {
        pns_hermes::PostOutcome::Status(self.0)
    }
}
fn destinations(status: u16) -> pns_application::Destinations<crate::HermesChannel<Reply>> {
    let mut destinations = pns_application::Destinations::new();
    destinations
        .register(crate::HermesChannel {
            post: Reply(status),
            key: Some("fixture-key".into()),
            url: "http://127.0.0.1:9/owned-fixture".into(),
            sync_deadline: None,
        })
        .unwrap();
    destinations
}
fn remote_input() -> LedgerSubmission {
    let mut input = submission();
    input.legs.truncate(1);
    input.legs[0].destination = "hermes".into();
    input.producer_request = Some("retained canonical request".into());
    input
}

mod claims;
mod failing;
mod failure_class;
mod failures;
mod outcomes;
mod prepare;
mod schema_migration;

mod atomicity;
mod processes;

mod completion;

mod journal;

mod metadata;

mod limits;

mod health;

mod http_retry;

fn reported(completion: &LedgerCompletion) -> pns_domain::Delivery {
    match completion {
        LedgerCompletion::Acknowledged { detail } => {
            pns_domain::Delivery::Delivered(detail.clone())
        }
        LedgerCompletion::Rejected { status, detail } => pns_domain::Delivery::Rejected {
            status: *status,
            detail: detail.clone(),
        },
        LedgerCompletion::Retry {
            outcome, detail, ..
        } => match outcome {
            UnconfirmedDelivery::Failed => pns_domain::Delivery::Failed(detail.clone()),
            UnconfirmedDelivery::Unlaunched => pns_domain::Delivery::Unlaunched(detail.clone()),
            UnconfirmedDelivery::Unknown => pns_domain::Delivery::Silent,
        },
    }
}
