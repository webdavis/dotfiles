use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn a_mapped_pulse_receives_the_moment_the_gate_read_after_delivery() {
    let now = Cell::new(599_u64);
    let decided_at = now.get();
    let observed = RefCell::new(Vec::new());
    let lights = Lights::default();
    // Delivery consumes the remaining minute before the lamp gate is reached.
    now.set(600);
    signal_after_delivery(
        &|| Some(now.get()),
        |at| u16::try_from(at).ok(),
        Some(&lights),
        || panic!("a routed lamp is not a room-based signal"),
        |_, now, minutes| observed.borrow_mut().push((now, minutes)),
    );
    assert_eq!(*observed.borrow(), [(Some(600), Some(600))]);
    assert_eq!(decided_at, 599);
}

#[test]
fn a_config_with_no_lamp_map_takes_the_plain_room_pulse_at_every_hour() {
    let flashed = Cell::new(false);
    signal_after_delivery(
        &|| Some(1_320 * 60),
        |at| u16::try_from(at / 60).ok(),
        None,
        || flashed.set(true),
        |_, _, _| panic!("no routing map"),
    );
    assert!(
        flashed.get(),
        "the dim window lives in `[lights]`, so a config without one has none to read"
    );
}

#[test]
fn a_mapped_pulse_with_no_clock_still_reaches_the_map() {
    let observed = RefCell::new(Vec::new());
    let lights = Lights::default();
    signal_after_delivery(
        &|| None,
        |_| panic!("no time to convert"),
        Some(&lights),
        || panic!("a routed lamp is not a room-based signal"),
        |_, now, minutes| observed.borrow_mut().push((now, minutes)),
    );
    assert_eq!(*observed.borrow(), [(None, None)]);
}
