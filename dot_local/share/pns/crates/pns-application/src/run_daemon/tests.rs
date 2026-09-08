use super::*;
mod fixture;
use fixture::World;

#[test]
fn a_disabled_daemon_never_prepares_the_spool_or_starts_a_tick() {
    let world = World::new(vec![Ok(false)]);
    assert_eq!(world.run(true), 0);
    assert_eq!(
        *world.log.borrow(),
        [
            "settings",
            "out:pns daemon: disabled in the config; exiting"
        ]
    );
}

#[test]
fn a_permanently_refused_spool_exits_cleanly_before_sleeping_or_reading_a_clock() {
    let world = World::new(vec![Ok(true)]);
    assert_eq!(world.run(false), 0);
    assert_eq!(
        *world.log.borrow(),
        ["settings", "prepare", "err:pns daemon: spool refused"]
    );
}

#[test]
fn the_daemon_reloads_on_the_thirtieth_tick_before_registering_or_draining() {
    let world = World::new(vec![Ok(true), Ok(true), Ok(false)]);
    assert_eq!(world.run(true), 0);
    assert_eq!(world.count("sleep"), 60);
    assert_eq!(world.count("clock"), 60);
    assert_eq!(world.count("reap"), 59);
    assert_eq!(world.count("presence"), 1);
    let log = world.log.borrow();
    let at = log.iter().position(|s| s == "presence").unwrap();
    assert_eq!(
        &log[at - 2..at + 4],
        [
            "clock",
            "settings",
            "presence",
            "schedule(100)",
            "reap",
            "heartbeat"
        ]
    );
    assert_eq!(
        log.last().unwrap(),
        "out:pns daemon: disabled in the config; exiting"
    );
}

#[test]
fn an_unreadable_daemon_config_warns_and_keeps_running_until_a_readable_off_switch() {
    let world = World::new(vec![Err("bad config".into()), Ok(false)]);
    assert_eq!(world.run(true), 0);
    assert_eq!(world.count("sleep"), 30);
    assert_eq!(world.count("reap"), 29);
    assert!(world.log.borrow().contains(
        &"err:pns daemon: the config could not be read (bad config); carrying on enabled".into()
    ));
}

#[test]
fn a_daemon_without_a_wall_clock_still_reaps_and_reloads_but_never_registers_a_poll() {
    let world = World {
        now: None,
        ..World::new(vec![Ok(true), Ok(true), Ok(false)])
    };
    assert_eq!(world.run(true), 0);
    assert_eq!(world.count("reap"), 59);
    assert_eq!(world.count("settings"), 3);
    assert_eq!(world.count("presence"), 0);
    assert_eq!(world.count("heartbeat"), 0);
}

#[test]
fn the_daemon_tick_accepts_both_bounds_and_refuses_the_adjacent_values() {
    for (raw, expected) in [
        (None, 1000),
        (Some("9"), 1000),
        (Some("10"), 10),
        (Some("11"), 11),
        (Some("59999"), 59999),
        (Some("60000"), 60000),
        (Some("60001"), 1000),
        (Some("garbage"), 1000),
    ] {
        assert_eq!(daemon_tick(raw), Duration::from_millis(expected), "{raw:?}");
    }
}

#[test]
fn presence_registration_keeps_a_future_due_and_cancels_when_the_sensor_is_off() {
    let world = World::new(Vec::new());
    crate::ensure_presence_poll(&world, Some(7), 100);
    let first = world.pending.borrow().clone().unwrap();
    assert_eq!((first.due, first.until, first.every), (100, 400, Some(7)));
    assert_eq!(first.args, ["presence", "poll", "--daemon"]);
    *world.pending.borrow_mut() = Some(pns_domain::jobs::Job { due: 500, ..first });
    crate::ensure_presence_poll(&world, Some(7), 101);
    let kept = world.pending.borrow().clone().unwrap();
    assert_eq!((kept.due, kept.until), (500, 500));
    crate::ensure_presence_poll(&world, None, 102);
    assert!(world.pending.borrow().is_none());
    assert_eq!(world.log.borrow().last().unwrap(), "cancel(presence)");
}
