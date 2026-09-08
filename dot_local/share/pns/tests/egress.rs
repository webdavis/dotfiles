use pns::channels::Event;
use pns::routing::ReportMode;
use pns_adapters::event_json;
use pns_protocol::{EgressEnvelope, EgressMode, RenderedEvent, RequestId};

#[test]
fn the_version_one_egress_body_is_byte_identical_to_the_executable_event() {
    let rich = Event {
        agent: "shell".into(),
        state: "done".into(),
        project: "雪".into(),
        branch: "feature/quoted".into(),
        detail: "a \"quote\"\n\t\\\0".into(),
        title: "shell · done · 雪".into(),
        message: "$(touch never); `echo inert`".into(),
        preview: "preview\r\n".into(),
        pane: "w1:p2".into(),
    };
    for event in [Event::default(), rich] {
        for (legacy_mode, mode) in [
            (ReportMode::Silent, EgressMode::Silent),
            (ReportMode::ReportOutcome, EgressMode::ReportOutcome),
        ] {
            let envelope = EgressEnvelope {
                request_id: RequestId::new("original-id").unwrap(),
                body: RenderedEvent {
                    agent: event.agent.clone(),
                    branch: event.branch.clone(),
                    detail: event.detail.clone(),
                    message: event.message.clone(),
                    mode,
                    pane: event.pane.clone(),
                    preview: event.preview.clone(),
                    project: event.project.clone(),
                    state: event.state.clone(),
                    title: event.title.clone(),
                },
            };
            let bytes = envelope.encode().unwrap();
            let wire: serde_json::Value = serde_json::from_str(&bytes).unwrap();
            let legacy = event_json(&event, legacy_mode);
            assert_eq!(wire["body"].to_string(), legacy);
            assert_eq!(serde_json::to_string(&envelope.body).unwrap(), legacy);
        }
    }
}
