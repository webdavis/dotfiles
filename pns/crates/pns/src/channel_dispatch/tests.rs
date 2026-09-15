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
        producer_request: None,
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
    // THE ROUTE SURVIVES THE OVERRIDE, which the URL alone cannot say: the
    // route names the signing key, so an override that also reset the route
    // would sign every captured post with the default route's key.
    //
    // AND THE DEFAULT ROUTE IS THE CONFIGURED NAME, never a compiled one:
    // these names are this case's own, so a deployment that renames its
    // routes is what the resolution is measured against.
    let routes = Routes::named("logbook", "sirens");
    for (route, resolved) in [
        ("", "logbook"),
        ("sirens", "sirens"),
        ("bad/route", "logbook"),
    ] {
        assert_eq!(
            hermes_target(route, Some("http://example.invalid/explicit"), &routes),
            (
                resolved.to_string(),
                "http://example.invalid/explicit".to_string()
            )
        );
    }
    for override_url in [None, Some("")] {
        // THE PATH FOLLOWS THE NAME AND THE GATEWAY DOES NOT MOVE: the
        // default URL's final segment is swapped for the configured route,
        // which is what keeps the route the key was granted to and the route
        // the URL names one value.
        assert_eq!(
            hermes_target("", override_url, &routes),
            (
                "logbook".to_string(),
                "http://127.0.0.1:8644/webhooks/logbook".to_string()
            )
        );
        assert_eq!(
            hermes_target("sirens", override_url, &routes),
            (
                "sirens".to_string(),
                "http://127.0.0.1:8644/webhooks/sirens".to_string()
            )
        );
        // AN UNUSABLE NAME FALLS BACK KEY AND ALL, so the post the default
        // route takes is signed with the default route's key rather than
        // refused for want of a key named `bad/route`.
        assert_eq!(
            hermes_target("bad/route", override_url, &routes),
            (
                "logbook".to_string(),
                "http://127.0.0.1:8644/webhooks/logbook".to_string()
            )
        );
    }
    // AND THE SHIPPED DEFAULT IS THE DEFAULT URL'S OWN LAST SEGMENT, which is
    // the one case where the two are allowed to be the same string.
    assert_eq!(
        hermes_target("", None, &Routes::default()),
        (
            Routes::default().default_route().to_string(),
            DEFAULT_HERMES_URL.to_string()
        )
    );
}

#[test]
fn the_discord_leg_is_built_with_the_default_route_the_config_named() {
    // THE MUTANT THIS PINS: the configured name replaced by a compiled one on
    // the way into the leg. It is the map key an event with NO PROJECT lands
    // on, and no assertion on the lookup itself can see the assignment: a
    // deployment that renamed its default route would lose every projectless
    // event to the catch-all with nothing red.
    let routes = Routes::named("logbook", "sirens");
    let leg = discord_channel(&pns_adapters::DiscordSettings::default(), "sirens", &routes);
    assert_eq!(leg.default_route, "logbook");
    assert_eq!(
        leg.route, "sirens",
        "the leg's own route is not the default"
    );
}

/// THE PHONE CARD IS UNCHANGED BY THE SENDER HEADER. An iOS notification
/// title shows roughly forty characters, and
/// `dotfiles · feat/pns-sender-header · blocked` is already past that, so a
/// header in the card title would cut off the one word the card exists to
/// deliver.
#[test]
fn the_card_title_stays_agent_state_project_while_the_sender_parts_ride_along() {
    let event = EventArgs {
        agent: "claude".into(),
        state: "blocked".into(),
        project: "dotfiles".into(),
        branch: "feat/pns-sender-header".into(),
        detail: "Bash(git push) needs approval".into(),
        session: "a1b2c3d4-dead-beef".into(),
        session_title: "arm posture alert".into(),
        ..EventArgs::default()
    };
    let rendered = rendered_event(&event, false);
    assert_eq!(rendered.title, "claude · blocked · dotfiles");
    assert_eq!(rendered.session, "a1b2c3d4-dead-beef");
    assert_eq!(rendered.session_title, "arm posture alert");
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
