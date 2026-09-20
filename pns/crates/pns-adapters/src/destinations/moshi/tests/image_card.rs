//! The per-card-type image opt-in, at the delivery seam: which card types
//! upload, what the card carries when one does, and every way an image card
//! falls back to the text one.

use super::*;
/// The card type `event()` raises, so a test arms exactly that one.
const ARMED: &str = "device_token = \"tok-1\"\n[image_cards]\ndone = true\n";

#[test]
fn a_card_type_nobody_armed_uploads_nothing_and_keeps_its_deep_link() {
    // SHIPPED OFF is the posture, and this is the whole of it: an empty
    // toggle table has to leave the channel byte for byte as it was.
    let channel = channel_over(
        RecordingHttp::uploading("abcde1xy"),
        "device_token = \"tok-1\"\n",
    );
    channel.deliver(&delivery_request(&event(), ReportMode::Silent));
    assert!(channel.http.uploads.lock().unwrap().is_empty());
    let posts = channel.http.posts.lock().unwrap();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].1.contains("\"type\":\"url\""), "{}", posts[0].1);
}

#[test]
fn an_armed_card_type_uploads_the_message_and_posts_an_image_card_in_its_place() {
    let channel = channel_over(RecordingHttp::uploading("abcde1xy"), ARMED);
    let armed = Event {
        state: "done".to_string(),
        ..event()
    };
    assert_eq!(
        channel.deliver(&delivery_request(&armed, ReportMode::Silent)),
        Delivery::Delivered("pushed the card with its image".to_string())
    );
    let uploads = channel.http.uploads.lock().unwrap();
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].0, "https://example.invalid/upload");
    assert_eq!(uploads[0].1, "tok-1", "the upload carried another token");
    assert!(uploads[0].2 > 0, "an empty image was uploaded");
    let posts = channel.http.posts.lock().unwrap();
    assert_eq!(posts.len(), 1, "the image card was posted more than once");
    assert_eq!(
        posts[0].1,
        image_body(
            "tok-1",
            &armed.title,
            &armed.preview,
            "https://i.getmoshi.app/abcde1xy",
            Some("original-42"),
        )
    );
    // THE TRADEOFF, PINNED: one `data`, one `type`, so the image card cannot
    // also carry the deep link this event's pane would have earned.
    assert!(!posts[0].1.contains("moshi://herdr"), "{}", posts[0].1);
}

#[test]
fn an_upload_nothing_came_back_from_falls_back_to_the_text_card_with_its_deep_link() {
    // THE MUTANT THIS PINS: a refused upload treated as a delivery, which
    // drops the event entirely on the one path an opt-in must never cost.
    let channel = channel_over(RecordingHttp::answering(true), ARMED);
    let armed = Event {
        state: "done".to_string(),
        ..event()
    };
    assert_eq!(
        channel.deliver(&delivery_request(&armed, ReportMode::Silent)),
        Delivery::Delivered("pushed the card".to_string())
    );
    assert_eq!(channel.http.uploads.lock().unwrap().len(), 1);
    let posts = channel.http.posts.lock().unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(
        posts[0].1,
        webhook_body(
            "tok-1",
            &armed.title,
            &armed.preview,
            herdr_link(&armed.pane).as_deref(),
            "original-42",
        )
    );
}

#[test]
fn a_webhook_that_refused_the_image_body_still_gets_the_text_card() {
    // The uploaded image is public and the card is what the operator sees,
    // so a refused image body is a card owed, not a card sent.
    let channel = channel_over(
        RecordingHttp {
            answers: false,
            ..RecordingHttp::uploading("abcde1xy")
        },
        ARMED,
    );
    let armed = Event {
        state: "done".to_string(),
        ..event()
    };
    assert_eq!(
        channel.deliver(&delivery_request(&armed, ReportMode::Silent)),
        Delivery::Failed(
            "push FAILED (the moshi endpoint refused it or could not be reached)".to_string()
        )
    );
    let posts = channel.http.posts.lock().unwrap();
    assert_eq!(posts.len(), 2, "the text card was never attempted");
    assert!(posts[0].1.contains("\"type\":\"image\""));
    assert!(posts[1].1.contains("moshi://herdr"), "{}", posts[1].1);
}

#[test]
fn a_message_with_nothing_to_draw_keeps_the_text_card_and_uploads_nothing() {
    let channel = channel_over(RecordingHttp::uploading("abcde1xy"), ARMED);
    let blank = Event {
        state: "done".to_string(),
        message: "  \n ".to_string(),
        ..event()
    };
    assert_eq!(
        channel.deliver(&delivery_request(&blank, ReportMode::Silent)),
        Delivery::Delivered("pushed the card".to_string())
    );
    assert!(channel.http.uploads.lock().unwrap().is_empty());
    assert_eq!(channel.http.posts.lock().unwrap().len(), 1);
}
