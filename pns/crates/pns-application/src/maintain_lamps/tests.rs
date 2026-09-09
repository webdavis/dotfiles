use super::*;
mod fixture;
use fixture::World;
use pns_domain::lights::phase::HeldEntry;

#[test]
fn an_inactive_house_forgets_old_complaints_without_starting_an_interval_or_renewing() {
    let world = World::empty();
    *world.complaint.borrow_mut() = "old complaint".into();
    world.run(Some(&Lights::default()), Some(100), true);
    let log = world.log.borrow();
    assert!(
        !log.iter()
            .any(|s| s == "interval" || s == "inventory" || s.starts_with("schedule("))
    );
    assert_eq!(log.last().map(String::as_str), Some("complaint-memory"));
    assert!(world.complaint.borrow().is_empty());
}

#[test]
fn shell_work_below_its_lamp_threshold_renews_the_tick_without_inheriting_an_agent_streak() {
    let world = World {
        shell: Some(95),
        ..World::empty()
    };
    let lights = Lights::default();
    world.run(Some(&lights), Some(100), true);
    let log = world.log.borrow();
    assert!(log.contains(&"streak(false)".into()));
    assert!(!log.contains(&"interval".into()));
    assert_eq!(
        log.last(),
        Some(&format!("schedule({},400,100)", 100 + lights.refresh_secs))
    );
}

#[test]
fn an_unreadable_tick_clock_clears_named_lamps_before_any_house_probe() {
    let world = World::empty();
    *world.held.borrow_mut() = Some(vec![HeldEntry::bare("light/a")]);
    world.run(Some(&Lights::default()), None, true);
    assert_eq!(
        *world.log.borrow(),
        [
            "clock",
            "held",
            "connect(None)",
            "write(light/a)",
            "remember-held"
        ]
    );
}

#[test]
fn missing_tick_credentials_preserve_held_state_and_do_not_probe_the_house() {
    let world = World::empty();
    *world.held.borrow_mut() = Some(vec![HeldEntry::bare("light/a")]);
    let lights = Lights::default();
    world.run(Some(&lights), Some(100), false);
    assert_eq!(
        *world.log.borrow(),
        [
            "clock".to_string(),
            format!("connect(Some({}))", lights.refresh_secs)
        ]
    );
    assert_eq!(
        world.held.borrow().as_deref(),
        Some([HeldEntry::bare("light/a")].as_slice())
    );
}

#[test]
fn a_tick_starts_its_interval_before_the_routing_readings_and_clears_only_named_lamps() {
    let world = World::empty();
    *world.held.borrow_mut() = Some(vec![HeldEntry::bare("light/a")]);
    world.run(Some(&Lights::default()), Some(100), true);
    let log = world.log.borrow();
    let interval = log
        .iter()
        .position(|s| s == "interval")
        .expect("an interval");
    assert_eq!(
        &log[interval..interval + 3],
        ["interval", "minutes", "presence"]
    );
    assert!(
        !log.contains(&"inventory".into()),
        "known held paths clear without GETs"
    );
    assert!(log.contains(&"write(light/a)".into()));
    assert_eq!(world.held.borrow().as_deref(), Some([].as_slice()));
}
