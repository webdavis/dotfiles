use super::*;
use crate::{ClaimedLeg, DestinationId, RetryDelivery, SubmissionIdentity};
use pns_domain::registry::Routing;
use pns_domain::routing::ReportMode;
use pns_domain::{Decision, Event};
use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
pub(super) enum Preparation {
    #[default]
    Created,
    Existing,
    Conflict,
    InvalidPlan,
    InvalidLease,
    Unavailable,
}

#[derive(Default)]
pub(super) struct Store {
    pub preparation: Preparation,
    pub fail_writes: bool,
    pub allow_retry: bool,
    pub retry: Mutex<Option<RetryDelivery<u64>>>,
    pub panic_on_delivery: bool,
    pub completed: Mutex<Vec<(u64, LedgerCompletion, u64)>>,
    pub notices: Mutex<Vec<String>>,
    pub steps: Mutex<Vec<String>>,
}

impl Store {
    pub fn note(&self, step: impl Into<String>) {
        self.steps.lock().unwrap().push(step.into());
    }
    pub fn steps(&self) -> Vec<String> {
        self.steps.lock().unwrap().clone()
    }
    pub fn notice(&self, message: &str) {
        self.note("notice");
        self.notices.lock().unwrap().push(message.into());
    }
}

impl DeliveryLedger for Store {
    type Claim = u64;
    fn prepare(
        &self,
        request: &LedgerSubmission,
        window: LeaseWindow,
    ) -> Result<PreparedSubmission<u64>, LedgerFailure> {
        self.note("prepare");
        assert_eq!(request, &submission());
        assert_eq!(window, lease());
        match self.preparation {
            Preparation::Created => Ok(PreparedSubmission::Created {
                sequence: 77,
                legs: request
                    .legs
                    .iter()
                    .enumerate()
                    .map(|(index, leg)| ClaimedLeg {
                        claim: index as u64,
                        leg: leg.clone(),
                    })
                    .collect(),
            }),
            Preparation::Existing => Ok(PreparedSubmission::Existing(Box::new(SubmissionRecord {
                sequence: 77,
                submission: submission(),
                attempts: vec![],
            }))),
            Preparation::Conflict => Err(LedgerFailure::ConflictingSubmission),
            Preparation::InvalidPlan => Err(LedgerFailure::InvalidPlan),
            Preparation::InvalidLease => Err(LedgerFailure::InvalidLease),
            Preparation::Unavailable => {
                Err(LedgerFailure::Unavailable("database is locked".into()))
            }
        }
    }
    fn record(
        &self,
        claim: &u64,
        completion: &LedgerCompletion,
        at: u64,
    ) -> Result<(), LedgerFailure> {
        self.note(format!("ledger:{claim}"));
        self.completed
            .lock()
            .unwrap()
            .push((*claim, completion.clone(), at));
        if self.fail_writes {
            Err(LedgerFailure::Unavailable("record failed".into()))
        } else {
            Ok(())
        }
    }
    fn claim_retry(
        &self,
        window: LeaseWindow,
        _limits: pns_domain::retry::RetryLimits,
    ) -> Result<Option<RetryDelivery<u64>>, LedgerFailure> {
        assert!(self.allow_retry, "submission must never retry inline");
        assert_eq!(window, lease());
        self.note("claim");
        Ok(self.retry.lock().unwrap().take())
    }
    fn inspect(&self, _: &SubmissionIdentity) -> Result<Option<SubmissionRecord>, LedgerFailure> {
        panic!("prepare owns duplicate detection")
    }
}

impl DecisionOutcomes for Store {
    fn begin(&self, identity: &SubmissionIdentity, record: &Record) -> Result<(), String> {
        self.note("begin");
        assert_eq!(*identity, submission().identity);
        assert!(record.legs.is_empty());
        assert_eq!(
            (record.permission_mode, record.agent_id, record.tool_name),
            ("original-mode", "original-agent", "original-tool")
        );
        if self.fail_writes {
            Err("begin failed".into())
        } else {
            Ok(())
        }
    }
    fn revise(
        &self,
        identity: &SubmissionIdentity,
        destination: &str,
        _: &Delivery,
    ) -> Result<bool, String> {
        self.note(format!("revise:{destination}"));
        assert_eq!(*identity, submission().identity);
        if self.fail_writes {
            Err("revise failed".into())
        } else {
            Ok(true)
        }
    }
}

pub(super) struct Destination<'a> {
    id: DestinationId,
    store: &'a Store,
    outcome: Delivery,
}

impl NotificationDestination for Destination<'_> {
    fn id(&self) -> &DestinationId {
        &self.id
    }
    fn capabilities(&self) -> Routing {
        Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        }
    }
    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        self.store.note(format!("deliver:{}", self.id.as_str()));
        assert_eq!(
            (request.producer, request.request_id, request.route),
            ("posture", Some("original-123"), "priority")
        );
        assert_eq!(*request.event, submission().event);
        assert_eq!(request.mode, ReportMode::ReportOutcome);
        assert!(!self.store.panic_on_delivery, "private transport panic");
        self.outcome.clone()
    }
}

pub(super) fn destinations(store: &Store, outcome: Delivery) -> Destinations<Destination<'_>> {
    let mut registry = Destinations::new();
    for name in ["alpha", "beta"] {
        registry
            .register(Destination {
                id: DestinationId::new(name),
                store,
                outcome: outcome.clone(),
            })
            .unwrap();
    }
    registry
}

pub(super) fn lease() -> LeaseWindow {
    LeaseWindow {
        now: 100,
        until: 130,
    }
}

pub(super) fn submission() -> LedgerSubmission {
    LedgerSubmission {
        producer_request: None,
        identity: SubmissionIdentity {
            producer: "posture".into(),
            request_id: "original-123".into(),
        },
        event: Event {
            title: "original event".into(),
            ..Event::default()
        },
        legs: ["alpha", "beta"]
            .into_iter()
            .map(|destination| LedgerLeg {
                destination: destination.into(),
                route: "priority".into(),
                mode: ReportMode::ReportOutcome,
                decorative: false,
            })
            .collect(),
    }
}

pub(super) fn decision() -> Decision {
    use pns_domain::{DecisionRequest, EnvironmentSnapshot};
    pns_domain::decide(
        &EnvironmentSnapshot::default(),
        &pns_domain::registry::Registry::default().all(),
        &Overrides::default(),
        DecisionRequest {
            silence_policy: pns_domain::SilencePolicy::Respect,
            scope: pns_domain::DeliveryScope::Automatic,
            pane: "",
            now_secs: Some(100),
            long_running: false,
            mobile_watch_card: false,
        },
    )
}
