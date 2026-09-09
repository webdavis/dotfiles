use super::*;
use pns_application::DeliveryRequest;

#[test]
fn the_original_request_id_rides_moshi_data_with_or_without_a_safe_action() {
    for original in ["original-42", "second-19"] {
        for (pane, action) in [
            ("wW:p21", Some("moshi://herdr?pane=wW:p21")),
            ("", None),
            ("bad&pane", None),
        ] {
            let channel = channel_with_settings(r#"token = "tok-1""#);
            let event = Event {
                pane: pane.into(),
                ..event()
            };
            let request = DeliveryRequest {
                producer: "upgrades",
                request_id: Some(original),
                event: &event,
                route: "priority",
                mode: ReportMode::Silent,
            };
            for _ in 0..2 {
                assert_eq!(
                    channel.deliver(&request),
                    Delivery::Delivered("pushed the card".into())
                );
            }
            let posts = channel.http.posts.lock().unwrap();
            assert_eq!(posts.len(), 2);
            for (_, body) in posts.iter() {
                let value: serde_json::Value = serde_json::from_str(body).unwrap();
                assert_eq!(value["data"]["request_id"], original);
                assert_eq!(value["data"].get("url").and_then(|v| v.as_str()), action);
                assert_eq!(
                    value["data"].get("type").and_then(|v| v.as_str()),
                    action.map(|_| "url")
                );
                assert_eq!(
                    value["data"].as_object().unwrap().len(),
                    if action.is_some() { 3 } else { 1 }
                );
                assert_eq!(value["token"], "tok-1");
                assert_eq!(value["message"], "a preview");
            }
        }
    }
}

pub(super) fn delivery_request(event: &Event, mode: ReportMode) -> DeliveryRequest<'_> {
    DeliveryRequest {
        producer: "test",
        request_id: Some("original-42"),
        event,
        route: "",
        mode,
    }
}
