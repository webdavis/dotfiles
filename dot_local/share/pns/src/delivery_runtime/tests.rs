use super::*;
use pns_adapters::ExecutableDestination;
use pns_application::{DeliveryLedger, DestinationId, Destinations};
use pns_domain::{
    Delivery,
    registry::{Registry, Routing},
};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn unavailable_identity_or_clock_keeps_the_owned_live_path_without_inventing_a_lease() {
    let root = crate::runtime_test_support::scratch("live-without-retention");
    let executable = root.join("channel");
    std::fs::write(&executable, format!(
        "#!/bin/sh\ncat >'{}'\nprintf '%s:%s' \"$PNS_PRODUCER\" \"${{PNS_REQUEST_ID-unavailable}}\" >'{}'\n",
        root.join("event").display(), root.join("identity").display(),
    )).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let routing = Routing {
        local: false,
        presence_gated: false,
        durable: true,
        event_dispatched: true,
    };
    let mut registry = Registry::new();
    registry.register_channel("owned-channel", routing).unwrap();
    let selection = registry.all();
    let mut destinations = Destinations::new();
    destinations
        .register(ExecutableDestination::new(
            DestinationId::new("owned-channel"),
            routing,
            executable,
            Duration::from_millis(200),
        ))
        .unwrap();
    let identity = SubmissionIdentity {
        producer: "posture".into(),
        request_id: "original-123".into(),
    };
    let event = EventArgs {
        detail: "original detail".into(),
        channel: "priority".into(),
        ..EventArgs::default()
    };
    let legs = [Leg {
        name: "owned-channel",
        mode: ReportMode::ReportOutcome,
        decorative: false,
    }];
    let store = SqliteStore::new(root.join("state"));
    let runtime = DeliveryRuntime {
        store: &store,
        selection: &selection,
        home: root.to_str().unwrap(),
        mobile: &Mobile::default(),
        hermes_key: None,
        json: false,
    };
    for (identity, now, expected) in [
        (None, Some(100), "pns:unavailable"),
        (Some(&identity), None, "posture:original-123"),
        (Some(&identity), Some(u64::MAX), "posture:original-123"),
    ] {
        let input = SubmissionInput {
            identity,
            producer_request: None,
            event: &event,
            legs: &legs,
            pane_dropped: false,
            record: None,
        };
        let result = runtime.attempt(&input, &|| now, &destinations);
        let Ok(Submitted::Attempted {
            sequence: None,
            outcomes,
        }) = result
        else {
            panic!("missing retention facts blocked the real live path: {result:?}")
        };
        assert!(matches!(outcomes.as_slice(), [(_, Delivery::Silent)]));
        assert_eq!(
            std::fs::read_to_string(root.join("identity")).unwrap(),
            expected
        );
        assert!(
            std::fs::read_to_string(root.join("event"))
                .unwrap()
                .contains("original detail")
        );
    }
    assert!(
        store.inspect(&identity).unwrap().is_none(),
        "no fabricated lease created a ledger row"
    );
}
