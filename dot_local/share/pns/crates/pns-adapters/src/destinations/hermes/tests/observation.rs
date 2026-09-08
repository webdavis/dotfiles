use super::*;
use crate::{BannerChannel, SqliteStore};
use pns_application::{
    CommandRunner, DeliveryLedger, Destinations, LeaseWindow, LedgerCompletion, LedgerLeg,
    LedgerSubmission, SubmissionDelivery, SubmissionIdentity, Submitted,
};

struct Notifier {
    answers: bool,
    calls: Mutex<Vec<Vec<String>>>,
}
impl CommandRunner for Notifier {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        assert_eq!(program, "terminal-notifier");
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| (*arg).into()).collect());
        self.answers.then(String::new)
    }
}

#[test]
fn a_stored_observation_retries_silently_with_full_multiline_detail_and_original_hermes_id() {
    let path = crate::state_fixtures::scratch("observation-retry")
        .canonicalize()
        .expect("the canonical scratch directory");
    let prefix = "disk drift\n🔒 private\n";
    let detail = format!("{prefix}{}", "x".repeat(1_900 - prefix.chars().count()));
    let input = LedgerSubmission {
        producer_request: None,
        identity: SubmissionIdentity {
            producer: "posture".into(),
            request_id: "heartbeat-42".into(),
        },
        event: Event {
            agent: "posture".into(),
            state: "observation".into(),
            detail: detail.clone(),
            message: detail.clone(),
            preview: "short preview".into(),
            title: "daily posture".into(),
            ..Event::default()
        },
        legs: vec![
            LedgerLeg {
                destination: "macos-banner".into(),
                route: "priority".into(),
                mode: ReportMode::Silent,
                decorative: true,
            },
            LedgerLeg {
                destination: "hermes".into(),
                route: "priority".into(),
                mode: ReportMode::Silent,
                decorative: false,
            },
        ],
    };
    let mut posted_bodies = Vec::new();
    for (now, answers) in [(10, false), (20, true)] {
        // Reopen the real ledger and reconstruct native destinations for retry.
        let store = SqliteStore::new(path.clone());
        let banner = BannerChannel {
            runner: Notifier {
                answers,
                calls: Mutex::new(Vec::new()),
            },
            terminal_id: "com.term".into(),
            herdr_path: None,
        };
        let mut hermes = channel_with_settings(
            "key = \"key\"",
            if answers {
                PostOutcome::Status(200)
            } else {
                PostOutcome::NoResponse
            },
        );
        hermes.url = super::super::channel_url(DEFAULT_HERMES_URL, "priority").unwrap();
        let mut destinations: Destinations<Box<dyn NotificationDestination + '_>> =
            Destinations::new();
        destinations.register(Box::new(&banner)).unwrap();
        destinations.register(Box::new(&hermes)).unwrap();
        let flow = SubmissionDelivery {
            ledger: &store,
            decisions: &store,
            destinations: &destinations,
        };
        let lease = LeaseWindow {
            now,
            until: now + 10,
        };
        let notice = |line: &str| panic!("unexpected storage notice: {line}");
        if !answers {
            let submitted = flow
                .submit(&input, None, lease, &|| Some(now), &notice)
                .unwrap();
            let Submitted::Attempted { outcomes, .. } = submitted else {
                panic!("a new event must attempt delivery")
            };
            assert_eq!(outcomes.len(), 2);
            assert!(
                outcomes
                    .iter()
                    .all(|(_, outcome)| matches!(outcome, Delivery::Failed(_)))
            );
        } else {
            assert_eq!(
                store.inspect(&input.identity).unwrap().unwrap().submission,
                input
            );
            for expected in ["macos-banner", "hermes"] {
                let (leg, outcome) = flow.retry(lease, &|| Some(now), &notice).unwrap().unwrap();
                assert_eq!(leg.destination, expected);
                assert_eq!(leg.route, "priority");
                assert!(matches!(outcome, Delivery::Delivered(_)), "{outcome:?}");
            }
            assert!(flow.retry(lease, &|| Some(now), &notice).unwrap().is_none());
        }
        let calls = banner.runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(
            !calls[0].iter().any(|arg| arg == "-sound"),
            "{now}: {:?}",
            calls[0]
        );
        assert_eq!(calls[0][3], "\\short preview");
        let posts = hermes.post.posts.lock().unwrap();
        assert_eq!(posts.len(), 1);
        let (url, body, signature, deadline, key) = &posts[0];
        assert_eq!(url, "http://127.0.0.1:8644/webhooks/priority");
        let decoded: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(decoded["detail"], detail);
        assert_eq!(decoded["detail"].as_str().unwrap().chars().count(), 1_900);
        assert_eq!(decoded["request_id"], "heartbeat-42");
        assert_eq!(decoded["state"], "observation");
        assert_eq!(key.as_deref(), Some("heartbeat-42"));
        assert_eq!(Some(signature.as_str()), sign("key", body).as_deref());
        assert_eq!(*deadline, Some(Duration::from_secs(10)));
        posted_bodies.push(body.clone());
    }
    assert_eq!(posted_bodies[0], posted_bodies[1]);
    let record = SqliteStore::new(path)
        .inspect(&input.identity)
        .unwrap()
        .unwrap();
    assert_eq!(record.attempts.len(), 4);
    assert!(
        record.attempts[2..]
            .iter()
            .all(|attempt| matches!(attempt.completion, LedgerCompletion::Acknowledged { .. }))
    );
}
