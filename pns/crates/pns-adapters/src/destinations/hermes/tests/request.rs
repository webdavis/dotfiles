use super::*;
use pns_application::DeliveryRequest;

fn request(event: &Event) -> DeliveryRequest<'_> {
    DeliveryRequest {
        producer: "upgrades",
        request_id: Some("original-42"),
        event,
        route: "priority",
        mode: ReportMode::ReportOutcome,
    }
}

#[test]
fn the_original_request_id_is_in_the_signed_hermes_body_on_every_attempt() {
    for original in ["original-42", "second-19"] {
        let channel = channel_with_settings(r#"key = "key""#, PostOutcome::Status(200));
        let event = event();
        let mut request = request(&event);
        request.request_id = Some(original);
        for _ in 0..2 {
            assert_eq!(
                channel.deliver(&request),
                Delivery::Delivered("posted HTTP 200".into())
            );
        }
        let posts = channel.post.posts.lock().unwrap();
        assert_eq!(posts.len(), 2);
        for (_, body, signature, deadline, _) in posts.iter() {
            let value: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(value["request_id"], original);
            assert_eq!(value["detail"], "the full message");
            assert_eq!(value.as_object().unwrap().len(), 5);
            assert_eq!(Some(signature.as_str()), sign("key", body).as_deref());
            assert_eq!(*deadline, Some(Duration::from_secs(5)));
        }
    }
}

#[test]
fn the_original_request_id_is_the_hermes_idempotency_key_on_every_attempt() {
    for original in ["original-42", "second-19"] {
        let channel = channel_with_settings(r#"key = "key""#, PostOutcome::Status(200));
        let event = event();
        let mut request = request(&event);
        request.request_id = Some(original);
        for _ in 0..2 {
            channel.deliver(&request);
        }
        let posts = channel.post.posts.lock().unwrap();
        assert_eq!(posts.len(), 2);
        for post in posts.iter() {
            assert_eq!(post.4.as_deref(), Some(original));
        }
    }
}

#[test]
fn an_unretained_hermes_attempt_omits_unavailable_identity_without_blocking_delivery() {
    let channel = channel_with_settings(r#"key = "key""#, PostOutcome::Status(200));
    let event = event();
    let mut request = request(&event);
    request.request_id = None;
    assert_eq!(
        channel.deliver(&request),
        Delivery::Delivered("posted HTTP 200".into())
    );
    let posts = channel.post.posts.lock().unwrap();
    assert_eq!(posts.len(), 1);
    let (_, body, signature, _, key) = &posts[0];
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    assert!(value.get("request_id").is_none());
    assert!(key.is_none());
    assert_eq!(value["detail"], "the full message");
    assert_eq!(Some(signature.as_str()), sign("key", body).as_deref());
}
