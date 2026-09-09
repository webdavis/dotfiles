use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn the_pulse_gate_reads_the_clock_after_delivery_crosses_into_quiet_hours() {
    let now = Cell::new(599_u64);
    let decided_at = now.get();
    let calls = RefCell::new(Vec::new());
    // Delivery consumes the remaining minute before the lamp gate is reached.
    now.set(600);
    signal_after_delivery(
        &|| Some(now.get()),
        |at| {
            calls.borrow_mut().push(at);
            u16::try_from(at).ok()
        },
        None,
        || Ok(pns_domain::lamps::parse_window("10:00-11:00")),
        || panic!("delivery crossed into quiet hours"),
        |_, _, _| panic!("no routing map"),
        |_| panic!("valid window"),
    );
    assert_eq!(*calls.borrow(), [600]);
    assert_eq!(decided_at, 599);
    now.set(599);
    let flashed = Cell::new(false);
    signal_after_delivery(
        &|| Some(now.get()),
        |at| u16::try_from(at).ok(),
        None,
        || Ok(pns_domain::lamps::parse_window("10:00-11:00")),
        || flashed.set(true),
        |_, _, _| panic!("no routing map"),
        |_| panic!("valid window"),
    );
    assert!(
        flashed.get(),
        "the same window permits the preceding minute"
    );
}

#[test]
fn a_mapped_pulse_receives_the_fresh_moment_without_consulting_the_legacy_window() {
    let observed = RefCell::new(Vec::new());
    let lights = Lights::default();
    signal_after_delivery(
        &|| Some(700),
        |_| Some(42),
        Some(&lights),
        || panic!("legacy window does not govern a routed lamp"),
        || panic!("a routed lamp is not a room-based signal"),
        |_, now, minutes| observed.borrow_mut().push((now, minutes)),
        |_| panic!("no refusal"),
    );
    assert_eq!(*observed.borrow(), [(Some(700), Some(42))]);
}

#[test]
fn an_unreadable_legacy_window_refuses_visibly_and_an_unreadable_clock_stays_dark() {
    let warnings = RefCell::new(Vec::new());
    signal_after_delivery(
        &|| None,
        |_| panic!("no time to convert"),
        None,
        || Err("bad window".into()),
        || panic!("unreadable window cannot flash"),
        |_, _, _| panic!("no map"),
        |line| warnings.borrow_mut().push(line.to_string()),
    );
    assert_eq!(*warnings.borrow(), ["bad window"]);
    signal_after_delivery(
        &|| None,
        |_| panic!("no time to convert"),
        None,
        || Ok(pns_domain::lamps::parse_window("10:00-11:00")),
        || panic!("no clock inside a requested quiet window"),
        |_, _, _| panic!("no map"),
        |_| panic!("parsed window"),
    );
}
