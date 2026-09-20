use super::*;
use pns_domain::{
    lamps::{
        Inventory, Lamp,
        config::{Behaviour, Target},
    },
    lights::{mute::Muted, unread::News},
};

fn status_world() -> World {
    World {
        blocked: vec![90],
        inventory: Inventory {
            lamps: ["status", "ordinary"]
                .map(|id| Lamp {
                    id: id.into(),
                    name: id.into(),
                    room: Some("room".into()),
                    zones: vec![],
                })
                .to_vec(),
            ..Inventory::default()
        },
        ..World::empty()
    }
}

fn lights() -> Lights {
    Lights {
        lamps: [(
            "status".into(),
            Target {
                behaviours: Some(vec![
                    Behaviour::Blocked,
                    Behaviour::Unseen,
                    Behaviour::Looping,
                ]),
                ..Target::default()
            },
        )]
        .into(),
        ..Lights::default()
    }
}

fn assert_fading(world: &World) {
    let writes = world.writes.borrow();
    assert!(!writes.is_empty(), "a status effect must actually start");
    assert!(
        writes
            .iter()
            .all(|(path, kind)| path == "light/status" && *kind == "fade")
    );
}

#[test]
fn silence_clears_an_active_effect_and_resumes_without_touching_ordinary_lamps() {
    let world = status_world();
    let lights = lights();
    world.run(Some(&lights), Some(100), true);
    assert_fading(&world);
    assert!(world.held.borrow().as_ref().unwrap()[0].resume.is_some());

    world.silenced.set(true);
    world.writes.borrow_mut().clear();
    world.log.borrow_mut().clear();
    world.run(Some(&lights), Some(101), true);
    assert_eq!(*world.writes.borrow(), [("light/status".into(), "clear")]);
    assert_eq!(world.held.borrow().as_deref(), Some([].as_slice()));
    assert!(!world.log.borrow().contains(&"inventory".into()));
    assert!(
        world
            .log
            .borrow()
            .contains(&format!("schedule({},401,101)", 101 + lights.refresh_secs))
    );

    world.writes.borrow_mut().clear();
    world.run(Some(&lights), Some(102), true);
    assert!(world.writes.borrow().is_empty());
    world.silenced.set(false);
    world.run(Some(&lights), Some(103), true);
    assert_fading(&world);
}

#[test]
fn silence_suppresses_each_persistent_state_without_losing_its_tick_lease() {
    for (blocked, shell, news) in [
        (vec![90], None, News::default()),
        (vec![], Some(1), News::default()),
        (
            vec![],
            None,
            News {
                done_at: Some(101),
                failed_at: None,
            },
        ),
        (
            vec![],
            None,
            News {
                done_at: None,
                failed_at: Some(101),
            },
        ),
    ] {
        let world = World {
            blocked,
            shell,
            news,
            ..status_world()
        };
        world.silenced.set(true);
        world.run(Some(&lights()), Some(1000), true);
        assert!(world.writes.borrow().is_empty());
        assert!(!world.log.borrow().contains(&"inventory".into()));
        assert!(
            world
                .log
                .borrow()
                .contains(&"schedule(1012,1300,1000)".into())
        );
        world.silenced.set(false);
        world.run(Some(&lights()), Some(1001), true);
        assert_fading(&world);
    }
}

#[test]
fn ending_silence_preserves_manual_room_mutes_and_their_expiry() {
    let world = status_world();
    world.silenced.set(true);
    world.mutes.borrow_mut().0.push(Muted {
        place: "room".into(),
        expiry: 150,
    });
    world.run(Some(&lights()), Some(100), true);
    world.silenced.set(false);
    world.run(Some(&lights()), Some(149), true);
    assert!(world.writes.borrow().is_empty());
    world.run(Some(&lights()), Some(150), true);
    assert_fading(&world);
}

#[test]
fn ending_silence_does_not_override_an_unreadable_manual_mute() {
    let world = status_world();
    world.silenced.set(true);
    world.mutes.borrow_mut().1.push("unreadable mute".into());
    world.run(Some(&lights()), Some(100), true);
    world.silenced.set(false);
    world.run(Some(&lights()), Some(101), true);
    assert!(world.writes.borrow().is_empty());
    assert_eq!(*world.complaint.borrow(), "unreadable mute");
}

#[test]
fn a_state_that_ends_during_silence_is_not_replayed() {
    let world = status_world();
    world.silenced.set(true);
    world.run(Some(&lights()), Some(100), true);
    world.silenced.set(false);
    world.log.borrow_mut().clear();
    world.run(Some(&lights()), Some(100_000), true);
    assert!(world.writes.borrow().is_empty());
    assert!(
        !world
            .log
            .borrow()
            .iter()
            .any(|line| line.starts_with("schedule("))
    );
}
