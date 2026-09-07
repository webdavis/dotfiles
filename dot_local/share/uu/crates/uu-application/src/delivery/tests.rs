use super::*;
use crate::ports::{MarkerSnapshot, RecordFailure, RunHeader};
use std::cell::RefCell;
use uu_domain::LaneReport;

struct SpyDelivery {
    outcome: RefCell<Option<RecordOutcome>>,
    alerts: RefCell<Vec<(bool, String)>>,
}

impl RunDelivery for SpyDelivery {
    fn alert(&self, target: AlertTarget<'_>, summary: &str) -> AlertOutcome {
        self.alerts
            .borrow_mut()
            .push((target == AlertTarget::Run, summary.to_string()));
        AlertOutcome::Delivered
    }

    fn record(&self, _record: RunRecord<'_>) -> RecordOutcome {
        self.outcome
            .borrow_mut()
            .take()
            .expect("one record per run")
    }
}

#[derive(Default)]
struct SpyPresentation(RefCell<Vec<String>>);

impl RunPresentation for SpyPresentation {
    fn header(&self, _epoch: i64, _marker: &MarkerSnapshot) -> RunHeader {
        unreachable!("delivery does not resample the header")
    }

    fn write_record(&self, _header: &RunHeader, _reports: &[LaneReport]) -> String {
        unreachable!("delivery does not rerender the record")
    }

    fn notice(&self, notice: Notice<'_>) {
        if let Notice::RecordPosted(description) = notice {
            self.0.borrow_mut().push(description.to_string());
        }
    }
}

fn record() -> RunRecord<'static> {
    RunRecord {
        failures: 0,
        deferred: 0,
        detail: "body",
    }
}

#[test]
fn a_refused_post_reports_failure_and_alerts_through_the_given_alerter() {
    let delivery = SpyDelivery {
        outcome: RefCell::new(Some(RecordOutcome::Rejected {
            url: "http://127.0.0.1:0/wherever".to_string(),
            cause: RecordFailure::NoResponse,
            description: "no response".to_string(),
        })),
        alerts: RefCell::default(),
    };
    let presentation = SpyPresentation::default();
    assert!(!deliver_record(&delivery, &presentation, record()));
    let calls = delivery.alerts.borrow();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].0, "the alert belongs to the whole run");
    assert!(
        calls[0].1.contains("http://127.0.0.1:0/wherever"),
        "{calls:?}"
    );
    assert_eq!(*presentation.0.borrow(), ["no response"]);
}

#[test]
fn a_delivered_post_reports_success_and_never_touches_the_alerter() {
    let delivery = SpyDelivery {
        outcome: RefCell::new(Some(RecordOutcome::Delivered {
            description: "posted".to_string(),
        })),
        alerts: RefCell::default(),
    };
    let presentation = SpyPresentation::default();
    assert!(deliver_record(&delivery, &presentation, record()));
    assert!(
        delivery.alerts.borrow().is_empty(),
        "{:?}",
        delivery.alerts.borrow()
    );
    assert_eq!(*presentation.0.borrow(), ["posted"]);
}
