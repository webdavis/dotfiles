use super::*;
use pns_application::DeliveryRequest;

#[test]
fn the_original_request_id_rides_a_card_that_has_an_action_to_carry_it() {
    for original in ["original-42", "second-19"] {
        let channel = channel_with_settings(r#"token = "tok-1""#);
        let event = Event {
            pane: "wW:p21".into(),
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
            assert_eq!(value["data"]["url"], "moshi://herdr?pane=wW:p21");
            assert_eq!(value["data"]["type"], "url");
            assert_eq!(value["data"].as_object().unwrap().len(), 3);
            assert_eq!(value["token"], "tok-1");
            assert_eq!(value["message"], "a preview");
        }
    }
}

#[test]
fn a_card_with_no_action_carries_no_data_object_at_all() {
    // MEASURED AGAINST THE LIVE ENDPOINT on 2026-09-09, after fifteen straight
    // failures of one queued card. `data` is a TAGGED UNION on moshi's side,
    // and a lone `request_id` matches none of its five members, so the whole
    // post comes back 422 "Expected union value" and no card is ever shown.
    // The request id was riding there for a tap to correlate, and a card with
    // no action has no tap, so it costs nothing to leave out and costs every
    // paneless notification to leave in.
    for pane in ["", "bad&pane"] {
        let channel = channel_with_settings(r#"token = "tok-1""#);
        let event = Event {
            pane: pane.into(),
            ..event()
        };
        let request = DeliveryRequest {
            producer: "upgrades",
            request_id: Some("original-42"),
            event: &event,
            route: "priority",
            mode: ReportMode::Silent,
        };
        assert_eq!(
            channel.deliver(&request),
            Delivery::Delivered("pushed the card".into())
        );
        let posts = channel.http.posts.lock().unwrap();
        let (_, body) = &posts[0];
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(value.get("data").is_none(), "{pane:?} still sent {body}");
        assert_eq!(value["token"], "tok-1");
        assert_eq!(value["title"], "claude done: dotfiles");
        assert_eq!(value["message"], "a preview");
        assert_eq!(value.as_object().unwrap().len(), 3);
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
