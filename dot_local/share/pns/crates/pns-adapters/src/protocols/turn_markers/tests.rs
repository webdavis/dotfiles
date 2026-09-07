use super::*;
use crate::state_fixtures::{published_mode, scratch};

#[test]
fn the_first_turn_marker_is_private_and_a_later_prompt_keeps_its_bytes() {
    let state = scratch("turn-private");
    start_of_turn(&state, "one", Some(12));
    let path = state.join("session-one.start");
    assert_eq!(
        published_mode(&path),
        0o600,
        "a turn marker is private at creation"
    );
    start_of_turn(&state, "one", Some(20));
    assert_eq!(std::fs::read(path).unwrap(), b"12");
}

#[test]
fn a_dangling_turn_marker_never_writes_its_symlink_target() {
    let state = scratch("turn-link");
    let target = state.join("other");
    std::os::unix::fs::symlink(&target, state.join("session-one.start")).unwrap();
    start_of_turn(&state, "one", Some(12));
    assert!(
        !target.exists(),
        "exclusive marker creation must refuse an existing link"
    );
}

#[test]
fn the_turn_sweep_removes_only_markers_strictly_older_than_seven_days() {
    let state = scratch("turn-age");
    let retention = 7 * 24 * 60 * 60;
    let now = retention + 10;
    for (name, epoch) in [
        ("old", 9),
        ("edge", 10),
        ("recent", 11),
        ("future", now + 1),
    ] {
        std::fs::write(
            state.join(format!("session-{name}.start")),
            epoch.to_string(),
        )
        .unwrap();
    }
    sweep_turn_markers(&state, Some(now));
    assert!(
        !state.join("session-old.start").exists(),
        "old abandoned turns must expire"
    );
    for name in ["edge", "recent", "future"] {
        assert!(state.join(format!("session-{name}.start")).exists());
    }
}

#[test]
fn unreadable_clocks_and_unowned_names_do_not_expire_turn_markers() {
    let state = scratch("turn-unknown");
    for (name, raw) in [
        ("session-old.start", "1"),
        ("session-torn.start", "x"),
        ("elsewhere.start", "1"),
        ("session-old.claim.7", "1"),
    ] {
        std::fs::write(state.join(name), raw).unwrap();
    }
    sweep_turn_markers(&state, None);
    assert_eq!(
        std::fs::read_to_string(state.join("session-old.start")).unwrap(),
        "1"
    );
    sweep_turn_markers(&state, Some(1_000_000));
    for name in [
        "session-torn.start",
        "elsewhere.start",
        "session-old.claim.7",
    ] {
        assert!(state.join(name).exists());
    }
}
