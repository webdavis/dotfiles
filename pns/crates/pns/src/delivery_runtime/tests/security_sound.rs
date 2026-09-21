use super::*;
use pns_adapters::BannerChannel;
use pns_application::{CommandRunner, NotificationDestination};
use std::sync::Mutex;

#[derive(Default)]
struct Notifier {
    calls: Mutex<Vec<Vec<String>>>,
    fail_first: bool,
}

impl CommandRunner for &Notifier {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        assert_eq!(program, "terminal-notifier");
        let mut calls = self.calls.lock().unwrap();
        calls.push(args.iter().map(|arg| (*arg).to_owned()).collect());
        (!self.fail_first || calls.len() > 1).then(String::new)
    }
}

fn request(class: Option<&str>, state: &str) -> String {
    let mut request = pns_protocol::RequestEnvelope::new(
        pns_protocol::RequestId::new("page-123").unwrap(),
        pns_protocol::Name::new("posture").unwrap(),
        pns_protocol::State::from_word(state).expect("the tests state one of the six words"),
    );
    request.delivery_class = class.map(|name| pns_protocol::Name::new(name).unwrap());
    request.encode().unwrap()
}

fn banner(notifier: &Notifier) -> BannerChannel<&Notifier> {
    BannerChannel {
        runner: notifier,
        terminal_id: String::new(),
        herdr_path: None,
    }
}

fn sounds(notifier: &Notifier) -> Vec<Option<String>> {
    notifier
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|args| {
            args.windows(2)
                .find(|pair| pair[0] == "-sound")
                .map(|pair| pair[1].clone())
        })
        .collect()
}

#[test]
fn security_sound_survives_retained_and_unretained_delivery() {
    if crate::runtime_test_support::in_private_process() {
        return;
    }
    for (now, unavailable_store) in [
        (Some(100), false),
        (Some(100), true),
        (None, false),
        (Some(u64::MAX), false),
    ] {
        for (class, state, expected) in [
            (Some("security"), "blocked", Some("Sosumi")),
            (Some("security"), "observation", None),
            (Some("security"), "done", Some("default")),
            (Some("other"), "blocked", Some("default")),
            (None, "blocked", Some("default")),
        ] {
            let root = crate::runtime_test_support::scratch("security-sound");
            if unavailable_store {
                std::fs::write(root.join("state"), b"preserved").unwrap();
            }
            let store = SqliteStore::new(root.join("state"));
            let notifier = Notifier::default();
            let banner = banner(&notifier);
            let mut registry = Registry::new();
            registry
                .register_channel("banner", banner.capabilities())
                .unwrap();
            let selection = registry.all();
            let mut destinations = Destinations::new();
            destinations.register(banner).unwrap();
            let runtime = DeliveryRuntime {
                store: &store,
                selection: &selection,
                home: root.to_str().unwrap(),
                mobile: &Mobile::default(),
                hermes_keys: &pns_adapters::HermesKeys::default(),
                discord: &pns_adapters::DiscordSettings::default(),
                routes: &pns_domain::routes::Routes::default(),
                json: false,
            };
            let event = EventArgs {
                state: state.into(),
                ..Default::default()
            };
            let identity = SubmissionIdentity {
                producer: "posture".into(),
                request_id: "page-123".into(),
            };
            let encoded = request(class, state);
            let input = SubmissionInput {
                identity: Some(&identity),
                producer_request: Some(&encoded),
                event: &event,
                legs: &[Leg {
                    name: "banner",
                    mode: ReportMode::Silent,
                    decorative: false,
                }],
                pane_dropped: false,
                record: None,
            };
            let result = runtime.attempt(&input, &|| now, &destinations).unwrap();
            assert!(matches!(result, Submitted::Attempted { .. }));
            if unavailable_store {
                assert!(matches!(
                    result,
                    Submitted::Attempted { sequence: None, .. }
                ));
                assert_eq!(std::fs::read(root.join("state")).unwrap(), b"preserved");
            }
            assert_eq!(
                sounds(&notifier),
                [expected.map(str::to_owned)],
                "{class:?}/{state}/{now:?}"
            );
        }
    }
}

#[test]
fn security_sound_survives_a_failed_delivery_and_ledger_retry() {
    let root = crate::runtime_test_support::scratch("security-sound-retry");
    let store = SqliteStore::new(root.join("state"));
    let notifier = Notifier {
        fail_first: true,
        ..Default::default()
    };
    let mut destinations = Destinations::new();
    destinations.register(banner(&notifier)).unwrap();
    let workflow = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    let input = LedgerSubmission {
        identity: SubmissionIdentity {
            producer: "posture".into(),
            request_id: "page-123".into(),
        },
        producer_request: Some(request(Some("security"), "blocked")),
        event: pns_domain::Event {
            state: "blocked".into(),
            ..Default::default()
        },
        legs: vec![LedgerLeg {
            destination: "banner".into(),
            route: String::new(),
            mode: ReportMode::Silent,
            decorative: false,
        }],
    };
    workflow
        .submit(&input, None, lease(100).unwrap(), &|| Some(100), &|_| {})
        .unwrap();
    let retried = workflow
        .retry(lease(1000).unwrap(), &|| Some(1000), &|_| {})
        .unwrap();
    assert!(matches!(retried, Some((_, Delivery::Delivered(_)))));
    assert_eq!(
        sounds(&notifier),
        [Some("Sosumi".into()), Some("Sosumi".into())]
    );
}
