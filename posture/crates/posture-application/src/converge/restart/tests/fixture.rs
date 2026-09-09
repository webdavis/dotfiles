use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type Calls = Rc<RefCell<Vec<&'static str>>>;
pub(super) fn pid(value: u64) -> ParentPid {
    ParentPid::parse(&value.to_string()).unwrap()
}
pub(super) fn restarted() -> Restarted {
    Restarted {
        parent: pid(20),
        settled_for: Duration::from_secs(1),
    }
}

pub(super) struct Control {
    pub vendor: VendorPlist,
    pub reject: Option<&'static str>,
    calls: Calls,
}
impl Control {
    fn command(&mut self, name: &'static str) -> Result<(), InspectionFailure> {
        self.calls.borrow_mut().push(name);
        if self.reject == Some(name) {
            Err(InspectionFailure::Failed)
        } else {
            Ok(())
        }
    }
}
impl OsqueryControl for Control {
    fn vendor_plist(&mut self) -> VendorPlist {
        self.calls.borrow_mut().push("vendor");
        self.vendor
    }
    fn config_check(&mut self) -> Result<(), InspectionFailure> {
        self.command("check")
    }
    fn stop(&mut self) -> Result<(), InspectionFailure> {
        self.command("stop")
    }
    fn start(&mut self) -> Result<(), InspectionFailure> {
        self.command("start")
    }
}
pub(super) struct Processes {
    pub previous: Option<ParentPid>,
    pub readings: Vec<(u64, Option<ParentPid>)>,
    pub failure: Option<usize>,
    pub observed: Vec<u64>,
    time: Rc<Cell<Duration>>,
    calls: Calls,
}
impl ProcessTable for Processes {
    fn daemon_parent(&mut self) -> Result<Option<ParentPid>, InspectionFailure> {
        self.calls.borrow_mut().push("parent");
        let index = self.observed.len();
        let now = self.time.get().as_millis() as u64;
        self.observed.push(now);
        if self.failure == Some(index) {
            return Err(InspectionFailure::TimedOut);
        }
        if index == 0 {
            return Ok(self.previous);
        }
        Ok(self
            .readings
            .iter()
            .rev()
            .find(|(at, _)| *at <= now)
            .unwrap()
            .1)
    }
}
pub(super) struct Time(Rc<Cell<Duration>>);
impl RestartClock for Time {
    fn elapsed(&self) -> Duration {
        self.0.get()
    }
    fn sleep(&mut self, duration: Duration) {
        assert_eq!(duration, Duration::from_millis(250));
        assert!(
            self.0.get() < Duration::from_secs(10),
            "the bounded loop did not finish"
        );
        self.0.set(self.0.get() + duration);
    }
}
pub(super) struct Fixture {
    pub calls: Calls,
    pub control: Control,
    pub processes: Processes,
    pub clock: Time,
}
impl Fixture {
    pub fn new() -> Self {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let time = Rc::new(Cell::new(Duration::ZERO));
        Self {
            calls: calls.clone(),
            control: Control {
                vendor: VendorPlist::Regular,
                reject: None,
                calls: calls.clone(),
            },
            processes: Processes {
                previous: Some(pid(10)),
                readings: vec![(0, Some(pid(20)))],
                failure: None,
                observed: Vec::new(),
                time: time.clone(),
                calls,
            },
            clock: Time(time),
        }
    }
    pub fn run(&mut self) -> Result<Restarted, RestartFailure> {
        restart_daemon(
            &mut self.control,
            &mut self.processes,
            &mut self.clock,
            RestartBounds::parse(Some("1"), Some("1")),
        )
    }
}
