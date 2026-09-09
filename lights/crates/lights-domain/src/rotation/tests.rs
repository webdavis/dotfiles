use super::*;
const SCENES: [&str; 4] = ["Dimmed", "Read", "Energize", "Concentrate"];
fn rotation() -> Rotation {
    Rotation::new(SCENES.map(str::to_owned).to_vec(), "Read".into()).unwrap()
}
#[test]
fn rotation_next_and_previous_from_each_index() {
    let rotation = rotation();
    for (i, name) in SCENES.iter().enumerate() {
        assert_eq!(rotation.next(Some(name)), SCENES[(i + 1) % 4]);
        assert_eq!(rotation.previous(Some(name)), SCENES[(i + 3) % 4]);
    }
}
#[test]
fn previous_from_zero_wraps_to_last() {
    assert_eq!(rotation().previous(Some("Dimmed")), "Concentrate");
}
#[test]
fn next_from_last_wraps_to_first() {
    assert_eq!(rotation().next(Some("Concentrate")), "Dimmed");
}
#[test]
fn missing_or_unlisted_scene_uses_read_fallback() {
    for current in [None, Some("Unlisted")] {
        assert_eq!(rotation().next(current), "Read");
        assert_eq!(rotation().previous(current), "Read");
    }
}
#[test]
fn both_halo_scenes_use_fallback_in_both_directions() {
    for halo in ["CC Halo Daylight", "CC Halo Amber"] {
        assert_eq!(rotation().next(Some(halo)), "Read");
        assert_eq!(rotation().previous(Some(halo)), "Read");
    }
}
#[test]
fn empty_rotation_is_rejected() {
    assert!(Rotation::new(vec![], "Read".into()).is_err());
}
#[test]
fn fallback_outside_rotation_is_rejected() {
    assert!(Rotation::new(vec!["Dimmed".into()], "Read".into()).is_err());
}
