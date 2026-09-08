use super::*;
use crate::{MarkerSnapshot, StateWriteFailure, StreakKind, StreakSnapshot};
use std::cell::Cell;
use std::rc::Rc;

struct State {
    held: Rc<Cell<bool>>,
    refuse: bool,
}
struct Guard(Rc<Cell<bool>>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl RunState for State {
    type Guard = Guard;
    fn acquire(&self) -> Result<Self::Guard, LockFailure> {
        if self.refuse {
            return Err(LockFailure::Contended("owned contention".into()));
        }
        self.held.set(true);
        Ok(Guard(self.held.clone()))
    }
    fn prune_removed_lanes(&self, _: &[&str]) {
        panic!("bootstrap must not prune")
    }
    fn marker(&self) -> MarkerSnapshot {
        panic!("bootstrap must not sample a marker")
    }
    fn write_marker(&self, _: i64) -> Result<(), StateWriteFailure> {
        panic!("bootstrap must not write a marker")
    }
    fn streak(&self, _: &str, _: StreakKind) -> StreakSnapshot {
        panic!("bootstrap must not read a streak")
    }
    fn write_streak(&self, _: &str, _: StreakKind, _: u32) -> Result<(), StateWriteFailure> {
        panic!("bootstrap must not write a streak")
    }
}

#[test]
fn bootstrap_holds_the_run_lock_for_the_capability_and_releases_it_afterward() {
    let state = State {
        held: Rc::new(Cell::new(false)),
        refuse: false,
    };
    let result = bootstrap(&state, || {
        assert!(
            state.held.get(),
            "bootstrap capability ran without the run lock"
        );
        BootstrapOutcome::Reported(LaneReport::new("owned"))
    });
    assert_eq!(result, BootstrapOutcome::Reported(LaneReport::new("owned")));
    assert!(!state.held.get(), "bootstrap leaked its run lock");
}

#[test]
fn a_refused_bootstrap_lock_never_invokes_the_capability() {
    let state = State {
        held: Rc::new(Cell::new(false)),
        refuse: true,
    };
    let result = bootstrap(&state, || panic!("refused lock invoked bootstrap"));
    assert_eq!(
        result,
        BootstrapOutcome::LockRefused(LockFailure::Contended("owned contention".into()))
    );
}
