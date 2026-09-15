use super::*;
use serde_json::{Value, json};

fn accepted_over(args: &[&str], writes: usize) -> (lights::Response, Vec<Vec<u8>>) {
    let mut responses = vec![(200, fixture())];
    responses.extend((0..writes).map(|_| (200, json!({"errors":[],"data":[]}))));
    command(args, Some(config()), responses)
}

#[test]
fn over_puts_the_bridge_transition_duration_in_the_brightness_body() {
    for (spelling, millis) in [("750ms", 750), ("2s", 2000), ("5m", 300_000)] {
        let (r, w) = accepted_over(&["--over", spelling, "brightness", "40"], 1);
        assert_eq!(r.exit, 0, "{spelling}");
        assert_eq!(
            body(&w[1]),
            json!({"dimming":{"brightness":40},"dynamics":{"duration":millis}})
        );
    }
}

#[test]
fn over_puts_the_duration_in_the_scene_recall() {
    let (r, w) = accepted_over(&["--over", "3s", "scene", "Read"], 1);
    assert_eq!(r.exit, 0);
    assert_eq!(
        body(&w[1]),
        json!({"recall":{"action":"active","duration":3000}})
    );
}

#[test]
fn no_flag_leaves_the_request_bodies_exactly_as_they_were() {
    let (_, w) = accepted_over(&["brightness", "40"], 1);
    assert_eq!(body(&w[1]), json!({"dimming":{"brightness":40}}));
    let (_, w) = accepted_over(&["scene", "Read"], 1);
    assert_eq!(body(&w[1]), json!({"recall":{"action":"active"}}));
}

#[test]
fn a_bad_duration_is_refused_by_name_before_the_bridge_is_read() {
    for spelling in ["2", "", "soon", "-3s", "2sec", "1h", "1.5s", "ms", "61m"] {
        let (r, w) = command(
            &["--over", spelling, "brightness", "40"],
            Some(config()),
            vec![],
        );
        assert_eq!(r.exit, 1, "{spelling}");
        assert!(r.stderr.contains("--over"), "{spelling}");
        assert!(w.is_empty(), "{spelling}");
    }
}

#[test]
fn over_is_refused_by_name_on_a_command_it_does_not_apply_to() {
    for args in [
        vec!["--over", "2s", "toggle"],
        vec!["--over", "2s", "on"],
        vec!["--over", "2s", "off"],
        vec!["--over", "2s", "status"],
        vec!["--over", "2s", "preset", "evening"],
    ] {
        let (r, w) = command(&args, Some(config()), vec![]);
        assert_eq!(r.exit, 1, "{args:?}");
        assert!(r.stderr.contains("--over"), "{args:?}");
        assert!(w.is_empty(), "{args:?}");
    }
}

#[test]
fn over_with_all_fades_every_room() {
    let (r, w) = accepted_over(&["--all", "--over", "2s", "brightness", "40"], 3);
    assert_eq!(r.exit, 0);
    assert_eq!(r.stderr, "");
    assert_eq!(w.len(), 4);
    for (request, group) in w[1..].iter().zip([4, 21, 2]) {
        let text = String::from_utf8_lossy(request);
        assert!(
            text.starts_with(&format!(
                "PUT /clip/v2/resource/grouped_light/00000000-0000-0000-0000-{group:012} "
            )),
            "{group}"
        );
        assert_eq!(
            serde_json::from_str::<Value>(text.split_once("\r\n\r\n").unwrap().1).unwrap(),
            json!({"dimming":{"brightness":40},"dynamics":{"duration":2000}}),
            "{group}"
        );
    }
}

#[test]
fn help_promises_over() {
    assert!(lights_protocol::HELP.contains("--over"));
}
