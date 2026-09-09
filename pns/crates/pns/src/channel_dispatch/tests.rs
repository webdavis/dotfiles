use super::*;
use pns_application::{DeliveryRequest, DestinationId};
use pns_domain::{
    Delivery,
    registry::{Registry, Routing},
    routing::ReportMode,
};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

const ROUTING: Routing = Routing {
    local: false,
    presence_gated: false,
    durable: true,
    event_dispatched: true,
};

struct Native {
    id: DestinationId,
    outcome: Delivery,
    calls: Arc<AtomicUsize>,
}

impl NotificationDestination for Native {
    fn id(&self) -> &DestinationId {
        &self.id
    }
    fn capabilities(&self) -> Routing {
        ROUTING
    }
    fn deliver(&self, _request: &DeliveryRequest<'_>) -> Delivery {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.outcome.clone()
    }
}

fn native(name: &'static str, outcome: Delivery) -> (Native, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        Native {
            id: DestinationId::new(name),
            outcome,
            calls: calls.clone(),
        },
        calls,
    )
}

fn fixture(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "pns-registration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let channel = root.join(format!("{name}.sh"));
    std::fs::write(&channel, "#!/bin/sh\n/bin/cat > \"${0%/*}/body\"\nprintf '%s' \"$PNS_REQUEST_ID\" > \"${0%/*}/id\"\nprintf '%s' \"$PNS_PRODUCER\" > \"${0%/*}/producer\"\nexit 9\n").unwrap();
    std::fs::set_permissions(channel, std::fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn request(event: &Event) -> DeliveryRequest<'_> {
    DeliveryRequest {
        producer: "fixture",
        request_id: Some("original-92"),
        event,
        route: "priority",
        mode: ReportMode::Silent,
    }
}

#[test]
fn native_delivery_wins_without_an_override_even_when_it_fails() {
    for outcome in [
        Delivery::Delivered("native".into()),
        Delivery::Failed("native failed".into()),
    ] {
        let directory = fixture("example");
        let (native, calls) = native("example", outcome.clone());
        let selected = registration::choose(native, None, None, false);
        assert_eq!(selected.deliver(&request(&Event::default())), outcome);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(
            !directory.join("body").exists(),
            "native refusal is not an executable fallback"
        );
    }
}

#[test]
fn an_explicit_channels_dir_means_executables_win() {
    let directory = fixture("example");
    let (native, calls) = native("example", Delivery::Delivered("native".into()));
    let selected = registration::choose(native, Some(&directory), None, false);
    assert_eq!(
        selected.deliver(&request(&Event::default())),
        Delivery::Silent
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        std::fs::read_to_string(directory.join("id")).unwrap(),
        "original-92"
    );
    let (native, calls) = self::native("missing", Delivery::Delivered("native".into()));
    let selected = registration::choose(native, Some(&directory), None, false);
    assert!(matches!(
        selected.deliver(&request(&Event::default())),
        Delivery::Unlaunched(_)
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn a_backend_refusal_prevents_both_native_and_executable_delivery() {
    for forced in [false, true] {
        let directory = fixture("mobile");
        let (native, calls) = native("mobile", Delivery::Delivered("native".into()));
        let selected = registration::choose(
            native,
            forced.then_some(directory.as_path()),
            Some("push SKIPPED, unsupported backend; nothing was sent".into()),
            false,
        );
        assert_eq!(
            selected.deliver(&request(&Event::default())),
            Delivery::Failed("push SKIPPED, unsupported backend; nothing was sent".into())
        );
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(
            !directory.join("body").exists(),
            "refusal must precede the executable override"
        );
    }
}

#[test]
fn a_new_registered_destination_dispatches_without_editing_a_name_switch() {
    let directory = fixture("new-plugin");
    let mut declarations = Registry::new();
    declarations
        .register_channel("new-plugin", ROUTING)
        .unwrap();
    declarations.register_sensor("sensor").unwrap();
    declarations
        .register_channel(
            "decoration",
            Routing {
                event_dispatched: false,
                ..ROUTING
            },
        )
        .unwrap();
    let (native, calls) = native("known", Delivery::Delivered("native".into()));
    declarations.register_channel("known", ROUTING).unwrap();
    let selected = registration::assemble(
        &declarations.all(),
        vec![Box::new(native)],
        &directory,
        false,
    );
    assert_eq!(
        selected.deliver("new-plugin", &request(&Event::default())),
        Delivery::Silent
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("id")).unwrap(),
        "original-92"
    );
    assert_eq!(
        selected.deliver("known", &request(&Event::default())),
        Delivery::Delivered("native".into())
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        selected
            .iter()
            .map(|entry| entry.id().as_str())
            .collect::<Vec<_>>(),
        ["new-plugin", "known"]
    );
    for name in ["sensor", "decoration"] {
        assert!(matches!(
            selected.deliver(name, &request(&Event::default())),
            Delivery::Unlaunched(_)
        ));
    }
}

#[test]
fn the_gateway_override_wins_and_blank_or_absent_overrides_keep_route_resolution() {
    for route in ["", "priority", "bad/route"] {
        assert_eq!(
            hermes_url_for(route, Some("http://example.invalid/explicit")),
            "http://example.invalid/explicit"
        );
    }
    for override_url in [None, Some("")] {
        assert_eq!(hermes_url_for("", override_url), DEFAULT_HERMES_URL);
        assert_eq!(
            hermes_url_for("priority", override_url),
            "http://127.0.0.1:8644/webhooks/priority"
        );
        assert_eq!(
            hermes_url_for("bad/route", override_url),
            DEFAULT_HERMES_URL
        );
    }
}

#[test]
fn rendering_drops_only_the_refused_pane_and_preserves_the_event_text() {
    let event = EventArgs {
        agent: "claude".into(),
        state: "done".into(),
        project: "pns".into(),
        branch: "topic".into(),
        detail: "details".into(),
        pane: "w1:p2".into(),
        ..EventArgs::default()
    };
    let mut expected = rendered_event(&event, false);
    assert_eq!(expected.pane, "w1:p2");
    assert_eq!(expected.title, "claude · done · pns");
    assert_eq!(expected.message, "topic: details");
    expected.pane.clear();
    assert_eq!(rendered_event(&event, true), expected);
}

mod environment;
