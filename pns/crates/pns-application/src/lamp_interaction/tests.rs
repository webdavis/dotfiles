use super::*;
use crate::{Clock, IdleProbe, PhoneInputProbe, PhoneMarkerProbe};
use std::cell::RefCell;

struct Probes {
    calls: RefCell<Vec<&'static str>>,
    clock: bool,
}
impl IdleProbe for Probes {
    fn idle_secs(&self) -> Option<u64> {
        self.calls.borrow_mut().push("idle");
        Some(100)
    }
}
impl PhoneInputProbe for Probes {
    fn phone_input_atime_secs(&self) -> Option<u64> {
        self.calls.borrow_mut().push("phone");
        Some(50)
    }
}
impl PhoneMarkerProbe for Probes {
    fn marker_mtime_secs(&self) -> Option<u64> {
        self.calls.borrow_mut().push("marker");
        Some(60)
    }
}
impl Clock for Probes {
    fn now_secs(&self) -> Option<u64> {
        self.calls.borrow_mut().push("clock");
        self.clock.then_some(200)
    }
}

#[test]
fn a_lamp_interaction_is_timed_after_all_three_actual_touch_samples() {
    let probes = Probes {
        calls: RefCell::new(Vec::new()),
        clock: true,
    };
    assert_eq!(last_lamp_interaction(&probes), Some(100));
    assert_eq!(*probes.calls.borrow(), ["idle", "phone", "marker", "clock"]);
}

#[test]
fn an_unreadable_clock_never_invents_a_lamp_interaction_at_epoch_zero() {
    let probes = Probes {
        calls: RefCell::new(Vec::new()),
        clock: false,
    };
    assert_eq!(last_lamp_interaction(&probes), None);
    assert_eq!(*probes.calls.borrow(), ["idle", "phone", "marker", "clock"]);
}
