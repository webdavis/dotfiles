use super::*;
use crate::PollSetting;
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
            "out:pns gateway: disabled in the config; exiting"
        ]
    );
}

#[test]
fn a_permanently_refused_spool_exits_cleanly_before_sleeping_or_reading_a_clock() {
    let world = World::new(vec![Ok(true)]);
    assert_eq!(world.run(false), 0);
    assert_eq!(
        *world.log.borrow(),
        ["settings", "prepare", "err:pns gateway: spool refused"]
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
    // THE GITHUB SOURCE IS ASKED ON THE SAME SWEEP, once, right after the
    // room sensor: both are the daemon's own jobs and both are re-registered
    // from one config read rather than two.
    assert_eq!(world.count("github"), 1);
    assert_eq!(world.count("calendar"), 1);
    let log = world.log.borrow();
    let at = log.iter().position(|s| s == "presence").unwrap();
    assert_eq!(
        &log[at - 2..at + 8],
        [
            "clock",
            "settings",
            "presence",
            "schedule(100)",
            "github",
            "cancel(github)",
            "calendar",
            "cancel(quiet-calendar)",
            "reap",
            "heartbeat"
        ]
    );
    assert_eq!(
        log.last().unwrap(),
        "out:pns gateway: disabled in the config; exiting"
    );
}

#[test]
fn an_unreadable_daemon_config_warns_and_keeps_running_until_a_readable_off_switch() {
    let world = World::new(vec![Err("bad config".into()), Ok(false)]);
    assert_eq!(world.run(true), 0);
    assert_eq!(world.count("sleep"), 30);
    assert_eq!(world.count("reap"), 29);
    assert!(world.log.borrow().contains(
        &"err:pns gateway: the config could not be read (bad config); carrying on enabled".into()
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
        (Some("9ms"), 1000),
        (Some("10ms"), 10),
        (Some("11ms"), 11),
        (Some("59999ms"), 59999),
        (Some("60s"), 60000),
        (Some("60001ms"), 1000),
        (Some("garbage"), 1000),
        // A BARE NUMBER IS NOT A TICK: it meant milliseconds here and seconds
        // to the reader beside it, which is the ambiguity the unit removes.
        (Some("500"), 1000),
    ] {
        assert_eq!(daemon_tick(raw), Duration::from_millis(expected), "{raw:?}");
    }
}

#[test]
fn presence_registration_keeps_a_future_due_and_cancels_when_the_sensor_is_off() {
    let world = World::new(Vec::new());
    crate::ensure_presence_poll(&world, PollSetting::Every(7), 100);
    let first = world.pending.borrow().clone().unwrap();
    assert_eq!((first.due, first.until, first.every), (100, 400, Some(7)));
    assert_eq!(first.args, ["presence", "poll", "--daemon"]);
    *world.pending.borrow_mut() = Some(pns_domain::jobs::Job { due: 500, ..first });
    crate::ensure_presence_poll(&world, PollSetting::Every(7), 101);
    let kept = world.pending.borrow().clone().unwrap();
    assert_eq!((kept.due, kept.until), (500, 500));
    crate::ensure_presence_poll(&world, PollSetting::Off, 102);
    assert!(world.pending.borrow().is_none());
    assert_eq!(world.log.borrow().last().unwrap(), "cancel(presence)");
}

#[test]
fn presence_registration_survives_a_config_the_daemon_cannot_read() {
    let world = World::new(Vec::new());
    crate::ensure_presence_poll(&world, PollSetting::Every(7), 100);
    let first = world.pending.borrow().clone().unwrap();
    *world.pending.borrow_mut() = Some(pns_domain::jobs::Job { due: 250, ..first });
    crate::ensure_presence_poll(&world, PollSetting::Unreadable, 200);
    let kept = world.pending.borrow().clone().unwrap();
    assert_eq!((kept.due, kept.until, kept.every), (250, 500, Some(7)));
    assert!(
        !world
            .log
            .borrow()
            .iter()
            .any(|line| line == "cancel(presence)")
    );
}

mod retries;
