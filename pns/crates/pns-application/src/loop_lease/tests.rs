use super::AcquireLoopLease;
use crate::LoopLeases;
use std::cell::RefCell;

#[derive(Default)]
struct Leases {
    steps: RefCell<Vec<String>>,
    fail: bool,
}
impl LoopLeases for Leases {
    fn begin(&self, pane: &str, now: u64) -> Result<(), String> {
        self.steps.borrow_mut().push(format!("begin:{pane}:{now}"));
        if self.fail {
            Err("write refused".into())
        } else {
            Ok(())
        }
    }
    fn end(&self, pane: &str) -> Result<(), String> {
        self.steps.borrow_mut().push(format!("end:{pane}"));
        if self.fail {
            Err("unlink refused".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn a_loop_lease_is_written_before_the_tick_is_registered() {
    let leases = Leases::default();
    AcquireLoopLease { leases: &leases }
        .begin("wW:p21", Some(100), |now| {
            leases.steps.borrow_mut().push(format!("register:{now}"))
        })
        .unwrap();
    assert_eq!(*leases.steps.borrow(), ["begin:wW:p21:100", "register:100"]);
}

#[test]
fn a_refused_loop_lease_is_not_registered_or_called_a_success() {
    let leases = Leases {
        fail: true,
        ..Default::default()
    };
    assert_eq!(
        AcquireLoopLease { leases: &leases }
            .begin("wW:p21", Some(100), |_| panic!("no registration")),
        Err("pns: loop: the lease could not be written: write refused".into())
    );
    assert_eq!(*leases.steps.borrow(), ["begin:wW:p21:100"]);
}

#[test]
fn a_loop_lease_needs_a_clock_before_it_touches_storage() {
    let leases = Leases::default();
    assert_eq!(
        AcquireLoopLease { leases: &leases }.begin("wW:p21", None, |_| panic!("no registration")),
        Err("pns: loop: the clock cannot be read; the lease was not taken".into())
    );
    assert!(leases.steps.borrow().is_empty());
}

#[test]
fn a_failed_loop_end_keeps_its_failure_visible() {
    let leases = Leases {
        fail: true,
        ..Default::default()
    };
    assert_eq!(AcquireLoopLease { leases: &leases }.end("wW:p21"),
        Err("pns: loop: the lease could not be given back (unlink refused); the loop lamp keeps breathing until it times out".into()));
    assert_eq!(*leases.steps.borrow(), ["end:wW:p21"]);
}
