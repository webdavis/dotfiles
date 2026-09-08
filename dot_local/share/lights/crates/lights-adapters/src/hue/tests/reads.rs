use super::super::*;
use super::*;

#[test]
fn bulk_room_reads_share_one_snapshot() {
    let (c, requests) = setup(vec![(200, fixture())]);
    assert!(c.room(&room()).unwrap().on);
    assert!(!c.room(&RoomName::new("Bedroom").unwrap()).unwrap().on);
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn room_without_grouped_light_is_malformed() {
    let mut data = fixture();
    data["data"][0]["services"] = json!([]);
    let (c, requests) = setup(vec![(200, data)]);
    assert!(matches!(
        c.room(&room()),
        Err(LightControlError::Malformed { .. })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn unknown_room_has_no_write() {
    let (c, requests) = setup(vec![(200, fixture())]);
    assert_eq!(
        c.room(&RoomName::new("Missing").unwrap()),
        Err(LightControlError::UnknownRoom {
            name: "Missing".into()
        })
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn successful_status_with_errors_is_refused() {
    let (c, _) = setup(vec![(
        200,
        json!({"errors":[{"description":"test-secret"}],"data":[]}),
    )]);
    assert!(matches!(
        c.room(&room()),
        Err(LightControlError::Refused { .. })
    ));
}
#[test]
fn data_with_errors_is_refused() {
    let mut data = fixture();
    data["errors"] = json!([{}]);
    let (c, _) = setup(vec![(200, data)]);
    assert!(matches!(
        c.room(&room()),
        Err(LightControlError::Refused { .. })
    ));
}
#[test]
fn missing_or_mistyped_envelope_arrays_are_malformed() {
    for data in [
        json!({}),
        json!({"errors":[],"data":{}}),
        json!({"errors":{},"data":[]}),
        json!({"errors":[],"data":null}),
    ] {
        let (c, _) = setup(vec![(200, data)]);
        assert!(matches!(
            c.room(&room()),
            Err(LightControlError::Malformed { .. })
        ));
    }
}
#[test]
fn malformed_resource_is_not_cached() {
    let mut data = fixture();
    data["data"][1]["on"] = json!({"on":"yes"});
    let (c, requests) = setup(vec![(200, data)]);
    for _ in 0..2 {
        assert!(matches!(
            c.room(&room()),
            Err(LightControlError::Malformed { .. })
        ));
    }
    assert_eq!(
        requests.lock().unwrap().len(),
        1,
        "a failed read must not be retried"
    );
}
#[test]
fn missing_dimming_is_not_zero() {
    let (c, _) = setup(vec![(200, fixture())]);
    assert_eq!(
        c.room(&RoomName::new("Bedroom").unwrap())
            .unwrap()
            .brightness,
        None
    );
}
#[test]
fn invalid_reported_brightness_is_malformed() {
    for brightness in [json!(-0.1), json!(100.1), json!("42"), json!(null)] {
        let mut data = fixture();
        data["data"][1]["dimming"]["brightness"] = brightness;
        let (c, _) = setup(vec![(200, data)]);
        assert!(matches!(
            c.room(&room()),
            Err(LightControlError::Malformed { .. })
        ));
    }
}
#[test]
fn duplicate_scene_names_do_not_cross_rooms() {
    let (c, _) = setup(vec![(200, fixture())]);
    let reference = c.room(&room()).unwrap().room;
    let scenes = c.scenes(&reference).unwrap();
    assert_eq!(
        scenes
            .iter()
            .find(|s| s.name == "Read")
            .unwrap()
            .scene
            .index(),
        5
    );
    assert_eq!(scenes.len(), 4);
}
#[test]
fn scene_name_is_scoped_to_room() {
    duplicate_scene_names_do_not_cross_rooms();
}
#[test]
fn bulk_room_and_scene_reads_share_snapshot() {
    let (c, requests) = setup(vec![(200, fixture())]);
    let reference = c.room(&room()).unwrap().room;
    c.scenes(&reference).unwrap();
    c.scenes(&reference).unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
}
#[test]
fn only_static_scene_drives_rotation() {
    let (c, _) = setup(vec![(200, fixture())]);
    let reference = c.room(&room()).unwrap().room;
    let active = c
        .scenes(&reference)
        .unwrap()
        .into_iter()
        .filter(|s| s.active)
        .map(|s| s.name)
        .collect::<Vec<_>>();
    assert_eq!(active, ["Read"]);
}
#[test]
fn incomplete_resources_are_refused_before_write() {
    for (index, key) in [
        (0, "metadata"),
        (0, "children"),
        (1, "owner"),
        (5, "group"),
        (5, "status"),
        (5, "actions"),
        (5, "speed"),
        (5, "auto_dynamic"),
    ] {
        let mut data = fixture();
        data["data"][index].as_object_mut().unwrap().remove(key);
        let (c, requests) = setup(vec![(200, data)]);
        assert!(
            matches!(c.room(&room()), Err(LightControlError::Malformed { .. })),
            "accepted {key}"
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}
