use super::*;
use pns_application::DeliveryRequest;
use pns_domain::{Event, routing::ReportMode};
use std::os::unix::fs::DirBuilderExt;
use std::sync::atomic::{AtomicU64, Ordering};

fn capture() -> (std::path::PathBuf, std::path::PathBuf) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "pns-egress-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let channel = root.join("channel.sh");
    std::fs::write(&channel, "#!/bin/sh\n/bin/cat > \"${0%/*}/body\"\nprintf '%s' \"$PNS_REQUEST_ID\" > \"${0%/*}/id\"\nprintf '%s' \"$PNS_PRODUCER\" > \"${0%/*}/producer\"\nexit 7\n").unwrap();
    std::fs::set_permissions(&channel, std::fs::Permissions::from_mode(0o700)).unwrap();
    (channel, root.join("body"))
}

/// THE PRODUCTION DEADLINE, and it has to be, because the assertions below read
/// what the child wrote. `deliver` bounds the write and the wait together, so a
/// child that outruns this budget is abandoned PART WAY THROUGH: the fixture
/// script writes `body`, then `id`, then `producer`, and the test then reads
/// files the child never reached.
///
/// This was 100ms, fifty times tighter than the five seconds
/// `channel_dispatch::EXECUTABLE_DEADLINE` gives a real channel, and no
/// assertion here is about the deadline at all. It held on an idle machine and
/// failed on a loaded CI runner, where spawning `/bin/sh`, `/bin/cat` and two
/// `printf`s took longer than a tenth of a second. The failure named the second
/// file (`id`, NotFound) rather than the budget, which is what made it read as
/// a race in the test rather than a timeout in the fixture.
///
/// Nothing waits this long when it passes: the wait ends when the child does,
/// measured at about 40ms locally.
const CHANNEL_DEADLINE: Duration = Duration::from_secs(5);

fn deliver(channel: &Path, request: &DeliveryRequest<'_>) -> Delivery {
    ExecutableDestination::new(
        DestinationId::new("fixture"),
        Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        },
        channel.to_owned(),
        CHANNEL_DEADLINE,
    )
    .deliver(request)
}

#[test]
fn an_executable_receives_the_original_id_and_exact_legacy_body_without_acknowledging_delivery() {
    for mode in [ReportMode::Silent, ReportMode::ReportOutcome] {
        for original in ["source-42", "other-19"] {
            let (channel, body) = capture();
            let event = Event {
                agent: "claude".into(),
                state: "done".into(),
                project: "pns".into(),
                detail: "quoted \"detail\"".into(),
                pane: "w1:p2".into(),
                branch: "topic".into(),
                title: "a title".into(),
                message: "the full message".into(),
                preview: "a preview".into(),
            };
            let request = DeliveryRequest {
                producer: "test",
                request_id: Some(original),
                event: &event,
                route: "priority",
                mode,
            };
            assert_eq!(
                deliver(&channel, &request),
                Delivery::Silent,
                "a launched exit7 is not acknowledged"
            );
            let bytes = std::fs::read(&body).unwrap();
            assert_eq!(bytes.last(), Some(&b'\n'));
            assert_eq!(
                bytes,
                format!("{}\n", crate::event_json(&event, mode)).as_bytes()
            );
            assert_eq!(
                std::fs::read_to_string(body.parent().unwrap().join("id")).unwrap(),
                original
            );
            assert_eq!(
                std::fs::read_to_string(body.parent().unwrap().join("producer")).unwrap(),
                "test"
            );
        }
    }
}

#[test]
fn an_unencodable_request_is_refused_before_the_executable_starts() {
    let (channel, body) = capture();
    let event = Event::default();
    for refused in ["", "has space", "line\nfeed"] {
        let request = DeliveryRequest {
            producer: "test",
            request_id: Some(refused),
            event: &event,
            route: "",
            mode: ReportMode::Silent,
        };
        assert!(matches!(
            deliver(&channel, &request),
            Delivery::Unlaunched(_)
        ));
        assert!(!body.exists(), "a refused envelope must not reach a child");
    }
}
