use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Destination {
    id: DestinationId,
    routing: Routing,
}

impl NotificationDestination for Destination {
    fn id(&self) -> &DestinationId {
        &self.id
    }

    fn capabilities(&self) -> Routing {
        self.routing
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        Delivery::Delivered(format!(
            "{}:{}:{}:{}:{}:{}",
            self.id.as_str(),
            request.producer,
            request.request_id.unwrap(),
            request.route,
            request.mode.as_str(),
            request.event.title
        ))
    }
}

fn destination(name: &'static str, durable: bool) -> Destination {
    Destination {
        id: DestinationId::new(name),
        routing: Routing {
            local: !durable,
            presence_gated: false,
            durable,
            event_dispatched: true,
        },
    }
}

#[test]
fn a_new_destination_dispatches_without_a_name_branch() {
    let mut destinations: Destinations<Box<dyn NotificationDestination>> = Destinations::new();
    destinations
        .register(Box::new(destination("new-destination", true)))
        .unwrap();
    let event = Event {
        title: "original title".into(),
        ..Event::default()
    };
    let request = DeliveryRequest {
        producer: "posture",
        request_id: Some("original-id"),
        event: &event,
        route: "security",
        mode: ReportMode::ReportOutcome,
    };
    assert_eq!(
        destinations.deliver("new-destination", &request),
        Delivery::Delivered(
            "new-destination:posture:original-id:security:sync:original title".into()
        )
    );
}

#[test]
fn a_duplicate_destination_does_not_replace_or_append_to_the_registry() {
    let mut destinations = Destinations::new();
    destinations
        .register(destination("original", false))
        .unwrap();
    assert_eq!(
        destinations.register(destination("original", true)),
        Err(RegistryError::Duplicate("original".into()))
    );
    assert_eq!(destinations.iter().count(), 1);
    assert!(!destinations.iter().next().unwrap().capabilities().durable);
}

#[test]
fn durable_selection_reads_capabilities_in_registration_order() {
    let mut destinations: Destinations<Box<dyn NotificationDestination>> = Destinations::new();
    for (name, durable) in [("local", false), ("archive", true), ("second", true)] {
        destinations
            .register(Box::new(destination(name, durable)))
            .unwrap();
    }
    assert_eq!(
        destinations.durable().map(|entry| entry.id().as_str()),
        Some("archive")
    );
    assert_eq!(
        destinations
            .iter()
            .map(|entry| entry.id().as_str())
            .collect::<Vec<_>>(),
        ["local", "archive", "second"]
    );
}

#[test]
fn compiled_destination_identifiers_cannot_escape_the_channel_directory() {
    for name in [
        "",
        "../hermes",
        "bad/name",
        "line\nbreak",
        "with space",
        ".",
        "💥",
    ] {
        assert!(
            std::panic::catch_unwind(|| DestinationId::new(name)).is_err(),
            "accepted {name:?}"
        );
    }
    assert_eq!(
        DestinationId::new("custom_2-plugin").as_str(),
        "custom_2-plugin"
    );
}

struct Reply<'a> {
    calls: &'a AtomicUsize,
    outcome: Option<Delivery>,
}

impl NotificationDestination for Reply<'_> {
    fn id(&self) -> &DestinationId {
        const ID: DestinationId = DestinationId::new("archive");
        &ID
    }

    fn capabilities(&self) -> Routing {
        destination("archive", true).routing
    }

    fn deliver(&self, _: &DeliveryRequest<'_>) -> Delivery {
        assert_eq!(self.calls.fetch_add(1, Ordering::SeqCst), 0);
        self.outcome
            .clone()
            .expect("private destination panic text")
    }
}

#[test]
fn every_delivery_verdict_is_recorded_after_dispatch_despite_a_sink_error() {
    let event = Event::default();
    let request = DeliveryRequest {
        producer: "posture",
        request_id: Some("original-id"),
        event: &event,
        route: "security",
        mode: ReportMode::ReportOutcome,
    };
    for outcome in [
        Delivery::Delivered("confirmed".into()),
        Delivery::Failed("refused".into()),
        Delivery::Unlaunched("not started".into()),
        Delivery::Silent,
    ] {
        let calls = AtomicUsize::new(0);
        let recorded = Recorded::new(
            Reply {
                calls: &calls,
                outcome: Some(outcome.clone()),
            },
            |received: &DeliveryRequest<'_>, verdict: &Delivery| {
                assert_eq!(calls.fetch_add(1, Ordering::SeqCst), 1);
                assert_eq!(received.producer, "posture");
                assert_eq!(received.request_id, Some("original-id"));
                assert_eq!(received.route, "security");
                assert_eq!(received.mode, ReportMode::ReportOutcome);
                assert!(std::ptr::eq(received.event, &event));
                assert_eq!(verdict, &outcome);
                Err("storage unavailable".to_string())
            },
        );
        assert_eq!(recorded.deliver(&request), outcome);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn a_panicking_destination_records_its_sanitized_failure() {
    let calls = AtomicUsize::new(0);
    let event = Event::default();
    let expected = Delivery::Failed("the archive channel PANICKED; nothing was sent".into());
    let recorded = Recorded::new(
        Reply {
            calls: &calls,
            outcome: None,
        },
        |_: &DeliveryRequest<'_>, verdict: &Delivery| {
            assert_eq!(calls.fetch_add(1, Ordering::SeqCst), 1);
            assert_eq!(verdict, &expected);
            Ok(())
        },
    );
    let request = DeliveryRequest {
        producer: "pns",
        request_id: Some("panic-id"),
        event: &event,
        route: "",
        mode: ReportMode::Silent,
    };
    assert_eq!(recorded.deliver(&request), expected);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn recording_preserves_the_registered_destination_and_durable_route() {
    let mut destinations = Destinations::new();
    destinations
        .register(Recorded::new(
            destination("archive", true),
            |_: &DeliveryRequest<'_>, _: &Delivery| Ok(()),
        ))
        .unwrap();
    let durable = destinations.durable().unwrap();
    assert_eq!(durable.id().as_str(), "archive");
    assert_eq!(durable.capabilities(), destination("archive", true).routing);
}
