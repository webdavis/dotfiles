use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct Memories {
    tick: RefCell<String>,
    quiet: RefCell<String>,
    log: RefCell<Vec<String>>,
}
impl LampComplaints for Memories {
    fn remembered(&self, kind: LampComplaint) -> String {
        match kind {
            LampComplaint::Tick => self.tick.borrow().clone(),
            LampComplaint::Quiet => self.quiet.borrow().clone(),
        }
    }
    fn remember(&self, kind: LampComplaint, said: Option<&str>) {
        self.log
            .borrow_mut()
            .push(format!("remember({})", said.unwrap_or("")));
        *match kind {
            LampComplaint::Tick => self.tick.borrow_mut(),
            LampComplaint::Quiet => self.quiet.borrow_mut(),
        } = said.unwrap_or("").into();
    }
}

#[test]
fn lamp_complaints_are_said_before_their_memory_is_changed() {
    let memory = Memories::default();
    report_lamp_complaints(
        &memory,
        LampComplaint::Tick,
        &["one".into(), "two".into()],
        |line| memory.log.borrow_mut().push(line.into()),
    );
    let log = memory.log.borrow();
    assert_eq!(&log[..2], ["one", "two"]);
    assert!(log[2].starts_with("remember("));
    assert_eq!(log.len(), 3);
}

#[test]
fn tick_and_event_complaints_remember_and_forget_independently() {
    let memory = Memories::default();
    for kind in [LampComplaint::Tick, LampComplaint::Quiet] {
        report_lamp_complaints(&memory, kind, &["same".into()], |line| {
            memory.log.borrow_mut().push(line.into())
        });
    }
    assert_eq!(
        memory
            .log
            .borrow()
            .iter()
            .filter(|line| *line == "same")
            .count(),
        2
    );
    memory.log.borrow_mut().clear();
    report_lamp_complaints(&memory, LampComplaint::Tick, &[], |_| {
        panic!("clear only forgets")
    });
    assert!(memory.tick.borrow().is_empty());
    assert!(!memory.quiet.borrow().is_empty());
    report_lamp_complaints(&memory, LampComplaint::Quiet, &["same".into()], |_| {
        panic!("quiet still remembers")
    });
    assert_eq!(memory.log.borrow().len(), 1);
    report_lamp_complaints(&memory, LampComplaint::Tick, &["same".into()], |line| {
        memory.log.borrow_mut().push(line.into())
    });
    assert_eq!(
        memory
            .log
            .borrow()
            .iter()
            .filter(|line| *line == "same")
            .count(),
        1
    );
}
