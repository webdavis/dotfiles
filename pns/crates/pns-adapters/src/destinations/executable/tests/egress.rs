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
        Duration::from_millis(100),
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
