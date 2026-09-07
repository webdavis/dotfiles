use super::*;
use std::cell::RefCell;

type PostCall = (String, String, String, Option<Duration>);

struct Post {
    calls: RefCell<Vec<PostCall>>,
    outcome: PostOutcome,
}
impl SignedPost for Post {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        self.calls
            .borrow_mut()
            .push((url.into(), body.into(), signature.into(), deadline));
        self.outcome
    }
}
struct Engine {
    calls: RefCell<Vec<(String, Vec<String>)>>,
    fail: bool,
}
impl Alerter for Engine {
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
        self.calls.borrow_mut().push((binary.into(), args.to_vec()));
        if self.fail {
            Err("engine refused".into())
        } else {
            Ok(())
        }
    }
}
fn records() -> Records {
    Records {
        url: "http://127.0.0.1:0/record".into(),
        key: "fixture-signing-key".into(),
        failure_webhook: Some("http://127.0.0.1:0/alarm".into()),
    }
}
fn delivery<'a>(
    records: Option<&'a Records>,
    engine: Option<&'a str>,
    fail: bool,
    outcome: PostOutcome,
) -> EngineRunDelivery<'a, Post, Engine> {
    EngineRunDelivery {
        records,
        engine,
        post: Post {
            calls: RefCell::default(),
            outcome,
        },
        alerter: Engine {
            calls: RefCell::default(),
            fail,
        },
    }
}

#[test]
fn every_alarm_posts_its_kind_host_and_detail_with_the_records_key_and_bound() {
    let records = records();
    let delivery = delivery(
        Some(&records),
        Some("fixture-pns"),
        false,
        PostOutcome::Status(204),
    );
    for (kind, state) in [
        (AlarmKind::Failed, "failed"),
        (AlarmKind::Stale, "stale"),
        (AlarmKind::Pending, "pending"),
        (AlarmKind::RecordLost, "record-lost"),
    ] {
        assert_eq!(
            delivery.alert(
                kind,
                "alarm-host",
                AlertTarget::Lane("chosen"),
                "unique detail"
            ),
            AlertOutcome::Delivered
        );
        let calls = delivery.post.calls.borrow();
        let call = calls.last().unwrap();
        assert_eq!(call.0, records.failure_webhook.as_ref().unwrap().as_str());
        let body: serde_json::Value = serde_json::from_str(&call.1).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"agent":"uu", "state":state, "project":"alarm-host", "detail":"unique detail"})
        );
        assert_eq!(call.2, sign(&records.key, &call.1).unwrap());
        assert_eq!(call.3, Some(Duration::from_secs(10)));
    }
    assert_eq!(delivery.post.calls.borrow().len(), 4);
    assert_eq!(delivery.alerter.calls.borrow().len(), 4);
    assert_eq!(
        delivery.alerter.calls.borrow()[0],
        (
            "fixture-pns".into(),
            alert_argv("alarm-host", "chosen", "unique detail")
        )
    );
}

#[test]
fn either_destination_failure_is_reported_after_both_were_attempted_once() {
    let records = records();
    for (engine_fails, outcome) in [
        (true, PostOutcome::Status(204)),
        (false, PostOutcome::Status(503)),
        (true, PostOutcome::NoResponse),
        (false, PostOutcome::NoStatus),
    ] {
        let delivery = delivery(Some(&records), Some("fixture-pns"), engine_fails, outcome);
        let result = delivery.alert(
            AlarmKind::Pending,
            "host",
            AlertTarget::Lane("chosen"),
            "detail",
        );
        assert!(matches!(result, AlertOutcome::Failed(_)), "{result:?}");
        assert_eq!(delivery.post.calls.borrow().len(), 1);
        assert_eq!(delivery.alerter.calls.borrow().len(), 1);
    }
}

#[test]
fn an_absent_failure_webhook_keeps_engine_only_delivery_and_never_posts() {
    let mut records = records();
    records.failure_webhook = None;
    let delivery = delivery(
        Some(&records),
        Some("fixture-pns"),
        false,
        PostOutcome::NoResponse,
    );
    assert_eq!(
        delivery.alert(AlarmKind::Failed, "host", AlertTarget::Run, "detail"),
        AlertOutcome::Delivered
    );
    assert!(delivery.post.calls.borrow().is_empty());
    assert_eq!(delivery.alerter.calls.borrow().len(), 1);
}

#[test]
fn a_webhook_without_an_engine_is_still_attempted_and_neither_means_not_configured() {
    let records = records();
    let webhook = delivery(Some(&records), None, false, PostOutcome::Status(200));
    assert_eq!(
        webhook.alert(AlarmKind::Stale, "host", AlertTarget::Run, "detail"),
        AlertOutcome::Delivered
    );
    assert_eq!(webhook.post.calls.borrow().len(), 1);
    assert!(webhook.alerter.calls.borrow().is_empty());
    let off = delivery(None, None, false, PostOutcome::Status(200));
    assert_eq!(
        off.alert(AlarmKind::Stale, "host", AlertTarget::Run, "detail"),
        AlertOutcome::NotConfigured
    );
    assert!(off.post.calls.borrow().is_empty());
    assert!(off.alerter.calls.borrow().is_empty());
}

#[test]
fn an_unsignable_alarm_does_not_post_or_suppress_the_engine_attempt() {
    let mut records = records();
    records.key.clear();
    let delivery = delivery(
        Some(&records),
        Some("fixture-pns"),
        false,
        PostOutcome::Status(200),
    );
    assert!(matches!(
        delivery.alert(AlarmKind::Failed, "host", AlertTarget::Run, "detail"),
        AlertOutcome::Failed(_)
    ));
    assert!(delivery.post.calls.borrow().is_empty());
    assert_eq!(delivery.alerter.calls.borrow().len(), 1);
}

#[test]
fn a_pending_record_passes_its_count_through_the_actual_delivery_adapter() {
    let records = records();
    let delivery = delivery(Some(&records), None, false, PostOutcome::Status(204));
    assert!(matches!(
        delivery.record(RunRecord {
            host: "record-host",
            failures: 0,
            deferred: 0,
            pending: 1,
            detail: "pending detail"
        }),
        RecordOutcome::Delivered { .. }
    ));
    let calls = delivery.post.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, records.url);
    let body: serde_json::Value = serde_json::from_str(&calls[0].1).unwrap();
    assert_eq!(body["state"], "pending");
    assert_eq!(body["detail"], "pending detail");
    assert!(delivery.alerter.calls.borrow().is_empty());
}
