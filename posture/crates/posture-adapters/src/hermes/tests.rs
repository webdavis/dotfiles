use super::*;
use posture_application::AlarmFailed;
use posture_domain::Severity;
use std::cell::RefCell;

#[derive(Default)]
struct Alarm {
    calls: Vec<(String, String)>,
}
impl IndependentAlarm for Alarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed> {
        self.calls.push((title.into(), detail.into()));
        Ok(())
    }
}

struct Posts {
    sent: RefCell<Vec<(String, String, String)>>,
    outcome: PostOutcome,
}
impl SignedPost for Posts {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        assert_eq!(deadline, Some(POST_DEADLINE));
        self.sent
            .borrow_mut()
            .push((url.into(), body.into(), signature_hex.into()));
        self.outcome
    }
}

fn alert(severity: Option<Severity>) -> Alert {
    Alert {
        occurrence_id: Some("occurrence-7".into()),
        event: "page",
        signal: posture_application::AlertSignal::NeedsAttention,
        severity,
        occurred_at: Some(1730000000),
        title: "Security finding".into(),
        detail: "line one".into(),
    }
}

fn keys() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("posture".to_string(), "key-posture".to_string()),
        ("priority".to_string(), "key-priority".to_string()),
    ])
}

fn subject(keys: BTreeMap<String, String>, outcome: PostOutcome) -> HermesWebhook<Posts, Alarm> {
    HermesWebhook::new(
        Posts {
            sent: RefCell::new(vec![]),
            outcome,
        },
        "http://127.0.0.1:8644/webhooks".to_string(),
        keys,
        Name::new("posture").unwrap(),
        Alarm::default(),
    )
}

#[test]
fn a_page_is_posted_to_its_tiers_route_signed_with_that_routes_own_key() {
    for (severity, route, key) in [
        (Some(Severity::Notice), "posture", "key-posture"),
        (Some(Severity::Critical), "priority", "key-priority"),
    ] {
        let mut sut = subject(keys(), PostOutcome::Status(204));
        assert_eq!(sut.submit(&alert(severity)), Submission::Accepted);
        let sent = sut.post.sent.borrow();
        let (url, body, signature) = sent.first().expect("one post");
        assert_eq!(url, &format!("http://127.0.0.1:8644/webhooks/{route}"));
        assert_eq!(signature, &sign(key, body).unwrap());
        assert_ne!(signature, &sign("key-wrong", body).unwrap());
        assert!(sut.alarm.calls.is_empty());
    }
}

#[test]
fn a_route_this_machine_holds_no_key_for_refuses_the_page_and_records_it() {
    for held in [
        BTreeMap::new(),
        BTreeMap::from([("posture".to_string(), String::new())]),
    ] {
        let mut sut = subject(held, PostOutcome::Status(204));
        assert_eq!(
            sut.submit(&alert(Some(Severity::Notice))),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
        assert!(sut.post.sent.borrow().is_empty(), "nothing may be posted");
        assert_eq!(
            sut.alarm.calls,
            vec![(
                ALARM_TITLE.to_string(),
                "Security finding\nline one".to_string()
            )]
        );
    }
}

#[test]
fn only_a_gateway_that_took_the_page_advances_acceptance() {
    for (outcome, failure) in [
        (PostOutcome::Status(401), Some(SubmissionFailure::Failed)),
        (PostOutcome::Status(500), Some(SubmissionFailure::Failed)),
        (PostOutcome::NoStatus, Some(SubmissionFailure::Failed)),
        (
            PostOutcome::NoResponse,
            Some(SubmissionFailure::Unavailable),
        ),
        (PostOutcome::Status(200), None),
        (PostOutcome::Status(299), None),
    ] {
        let mut sut = subject(keys(), outcome);
        let expected = match failure {
            Some(failure) => Submission::NotAccepted(failure),
            None => Submission::Accepted,
        };
        assert_eq!(
            sut.submit(&alert(Some(Severity::Notice))),
            expected,
            "{outcome:?}"
        );
        assert_eq!(sut.alarm.calls.len(), usize::from(failure.is_some()));
    }
}

#[test]
fn an_untiered_page_takes_the_route_the_sink_was_built_with() {
    let mut sut = subject(keys(), PostOutcome::Status(204));
    assert_eq!(sut.submit(&alert(None)), Submission::Accepted);
    assert_eq!(
        sut.post.sent.borrow()[0].0,
        "http://127.0.0.1:8644/webhooks/posture"
    );
}

#[test]
fn a_gateway_base_written_with_a_trailing_slash_does_not_double_it() {
    let mut sut = HermesWebhook::new(
        Posts {
            sent: RefCell::new(vec![]),
            outcome: PostOutcome::Status(204),
        },
        "http://127.0.0.1:8644/webhooks/".to_string(),
        keys(),
        Name::new("posture").unwrap(),
        Alarm::default(),
    );
    assert_eq!(sut.submit(&alert(None)), Submission::Accepted);
    assert_eq!(
        sut.post.sent.borrow()[0].0,
        "http://127.0.0.1:8644/webhooks/posture"
    );
}
