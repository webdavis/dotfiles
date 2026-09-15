use super::*;
use serde_json::{Value, json};

/// The three shipped aliases, in the order `--all` walks them.
const ROOMS: [&str; 3] = ["3F - MBedroom", "2F - Kitchen", "3F - Studio"];

fn accepted_all(args: &[&str], fixture: Value, writes: usize) -> (lights::Response, Vec<Vec<u8>>) {
    let mut responses = vec![(200, fixture)];
    responses.extend((0..writes).map(|_| (200, json!({"errors":[],"data":[]}))));
    command(args, Some(config()), responses)
}

#[test]
fn all_steps_every_configured_room_and_names_each_one() {
    let (r, w) = accepted_all(&["--all", "brightness", "up"], fixture(), 3);
    assert_eq!(r.exit, 0);
    assert_eq!(r.stderr, "");
    assert_eq!(
        r.stdout,
        "3F - MBedroom: brightness up\n2F - Kitchen: brightness up\n3F - Studio: brightness up\n"
    );
    // One bulk read, then one write per room.
    assert_eq!(w.len(), 4);
    for (request, group) in w[1..].iter().zip([4, 21, 2]) {
        assert!(
            String::from_utf8_lossy(request).starts_with(&format!(
                "PUT /clip/v2/resource/grouped_light/00000000-0000-0000-0000-{group:012} "
            )),
            "{group}"
        );
    }
}

#[test]
fn a_room_that_fails_leaves_the_others_done_and_sets_the_exit_code() {
    // Only the Studio carries the rotation in the shipped fixture, so the other
    // two rooms cannot reach the scene `next` names.
    let (r, w) = accepted_all(&["--all", "scene", "next"], fixture(), 1);
    assert_eq!(r.exit, 3);
    assert_eq!(r.stdout, "Room: 3F - Studio | Scene: Energize\n");
    assert_eq!(r.stderr.lines().count(), 2);
    assert!(r.stderr.contains("3F - MBedroom") && r.stderr.contains("2F - Kitchen"));
    assert_eq!(w.len(), 2);
}

/// The same bridge with a full rotation in every room, each room sitting on a
/// different scene of it.
fn fixture_with_a_rotation_per_room() -> Value {
    let mut fixture = fixture_with_no_active_scene();
    let template = fixture["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["type"] == "scene")
        .unwrap()
        .clone();
    let data = fixture["data"].as_array_mut().unwrap();
    // The Studio sits on its own Dimmed, so its step is the Read it already has.
    for resource in data.iter_mut() {
        if resource["id"] == json!("00000000-0000-0000-0000-000000000007") {
            resource["status"]["active"] = json!("static");
        }
    }
    let mut id = 30;
    for (room, active) in [("03", "Concentrate"), ("20", "Read")] {
        for name in ["Dimmed", "Read", "Energize", "Concentrate"] {
            let mut scene = template.clone();
            scene["id"] = json!(format!("00000000-0000-0000-0000-0000000000{id}"));
            scene["metadata"]["name"] = json!(name);
            scene["group"]["rid"] = json!(format!("00000000-0000-0000-0000-0000000000{room}"));
            scene["status"]["active"] = json!(if name == active { "static" } else { "inactive" });
            data.push(scene);
            id += 1;
        }
    }
    fixture
}

#[test]
fn all_advances_each_room_from_its_own_current_scene() {
    let (r, w) = accepted_all(
        &["--all", "scene", "next"],
        fixture_with_a_rotation_per_room(),
        3,
    );
    assert_eq!(r.exit, 0);
    assert_eq!(r.stderr, "");
    assert_eq!(
        r.stdout,
        "Room: 3F - MBedroom | Scene: Dimmed\n\
         Room: 2F - Kitchen | Scene: Energize\n\
         Room: 3F - Studio | Scene: Read\n"
    );
    assert_eq!(w.len(), 4);
    // Bedroom Concentrate wraps to its own Dimmed, Kitchen Read steps to its own
    // Energize, and the Studio steps from Dimmed to the Read it already had.
    for (request, scene) in w[1..].iter().zip([30, 36, 6]) {
        assert!(
            String::from_utf8_lossy(request).starts_with(&format!(
                "PUT /clip/v2/resource/scene/00000000-0000-0000-0000-{scene:012} "
            )),
            "{scene}"
        );
    }
}

#[test]
fn all_with_a_room_is_refused_before_the_bridge_is_read() {
    let (r, w) = command(
        &["--all", "--room", "studio", "scene", "Read"],
        Some(config()),
        vec![],
    );
    assert_eq!(r.exit, 1);
    assert!(r.stderr.contains("--all") && r.stderr.contains("--room"));
    assert!(w.is_empty());
}

#[test]
fn all_room_names_are_the_ones_the_help_promises() {
    assert!(lights_protocol::HELP.contains("--all"));
    let (r, _) = accepted_all(&["--all", "brightness", "50"], fixture(), 3);
    for room in ROOMS {
        assert!(r.stdout.contains(room), "{room}");
    }
}
