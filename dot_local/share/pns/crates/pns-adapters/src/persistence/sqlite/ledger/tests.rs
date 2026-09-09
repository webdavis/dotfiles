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
mod claims;
mod failures;
mod outcomes;
mod prepare;

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
