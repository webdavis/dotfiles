use super::*;
use lights_domain::{Brightness, Direction};

fn successful_write(
    run: impl FnOnce(&HueLightController, &RoomRef),
    expected_path: &str,
    expected_body: Value,
) {
    let (c, requests) = setup(vec![
        (200, fixture()),
        (200, json!({"errors":[],"data":[]})),
    ]);
    let room = c.room(&room()).unwrap().room;
    run(&c, &room);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let read = String::from_utf8_lossy(&requests[0]);
    assert!(read.starts_with("GET /clip/v2/resource HTTP/1.1\r\n"));
    assert!(read.contains("hue-application-key: test-secret\r\n"));
    let write = String::from_utf8_lossy(&requests[1]);
    assert!(write.starts_with(&format!(
        "PUT /clip/v2/resource/{expected_path} HTTP/1.1\r\n"
    )));
    let body = write.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(serde_json::from_str::<Value>(body).unwrap(), expected_body);
}
#[test]
fn power_body_matches_requested_state() {
    for on in [true, false] {
        successful_write(
            |c, r| c.set_power(r, on).unwrap(),
            &format!("grouped_light/{}", id(2)),
            json!({"on":{"on":on}}),
        );
    }
}
#[test]
fn absolute_body_uses_requested_level() {
    for n in [0, 1, 42, 101] {
        successful_write(
            |c, r| {
                c.set_brightness(r, BrightnessChange::Absolute(Brightness::new(n)))
                    .unwrap()
            },
            &format!("grouped_light/{}", id(2)),
            json!({"dimming":{"brightness":n.clamp(1,100)}}),
        );
    }
}
#[test]
fn relative_body_uses_direction_and_delta() {
    for (direction, action) in [(Direction::Up, "up"), (Direction::Down, "down")] {
        successful_write(
            |c, r| {
                c.set_brightness(
                    r,
                    BrightnessChange::Step {
                        direction,
                        percent: 5,
                    },
                )
                .unwrap()
            },
            &format!("grouped_light/{}", id(2)),
            json!({"dimming_delta":{"action":action,"brightness_delta":5}}),
        );
    }
}
#[test]
fn scene_recall_body_requests_active() {
    successful_write(
        |c, r| {
            let scene = c
                .scenes(r)
                .unwrap()
                .into_iter()
                .find(|s| s.name == "Read")
                .unwrap();
            c.set_scene(&scene.scene).unwrap();
        },
        &format!("scene/{}", id(6)),
        json!({"recall":{"action":"active"}}),
    );
}
#[test]
fn invalid_room_reference_does_not_write() {
    let (c, requests) = setup(vec![(200, fixture())]);
    c.room(&room()).unwrap();
    for index in [1, 99] {
        assert_eq!(
            c.set_power(&RoomRef::from_index(index), true),
            Err(LightControlError::InvalidReference)
        );
        assert_eq!(
            c.set_brightness(
                &RoomRef::from_index(index),
                BrightnessChange::Absolute(Brightness::new(1))
            ),
            Err(LightControlError::InvalidReference)
        );
    }
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn invalid_scene_reference_does_not_write() {
    let (c, requests) = setup(vec![(200, fixture())]);
    c.room(&room()).unwrap();
    for index in [0, 99] {
        assert_eq!(
            c.set_scene(&SceneRef::from_index(index)),
            Err(LightControlError::InvalidReference)
        );
    }
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn write_envelope_must_confirm_acceptance() {
    for (status, body) in [
        (200, json!({"errors":[{}],"data":[]})),
        (403, json!({"errors":[],"data":[]})),
        (200, json!({"errors":[]})),
    ] {
        let (c, requests) = setup(vec![(200, fixture()), (status, body)]);
        let room = c.room(&room()).unwrap().room;
        assert!(c.set_power(&room, true).is_err());
        assert_eq!(requests.lock().unwrap().len(), 2);
    }
}
