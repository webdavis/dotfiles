use crate::{DaemonSpool, JobChildren, SpoolReading};
use pns_domain::jobs::Job;
use std::{cell::RefCell, rc::Rc};

pub(super) type Trace = Rc<RefCell<Vec<String>>>;
pub(super) struct Spool {
    pub trace: Trace,
    pub peek: Option<Job>,
    pub owned: Option<Job>,
    pub claim: bool,
    pub hand_back: Result<bool, String>,
    pub release: Result<(), String>,
}
pub(super) struct Children {
    pub trace: Trace,
    pub running: bool,
    pub fail: bool,
}
pub(super) fn rig() -> (Spool, Children, Trace) {
    let trace = Trace::default();
    let job = Job {
        id: "job".into(),
        due: 100,
        until: 1000,
        every: Some(10),
        unless_marker: None,
        args: vec!["original".into()],
    };
    (
        Spool {
            trace: trace.clone(),
            peek: Some(job.clone()),
            owned: Some(job),
            claim: true,
            hand_back: Ok(true),
            release: Ok(()),
        },
        Children {
            trace: trace.clone(),
            running: false,
            fail: false,
        },
        trace,
    )
}
impl DaemonSpool for Spool {
    type Entry = u8;
    fn entries(&self) -> Vec<u8> {
        self.trace.borrow_mut().push("entries".into());
        vec![0]
    }
    fn id(&self, _: &u8) -> Option<String> {
        Some("job".into())
    }
    fn describe(&self, entry: &u8) -> String {
        format!("entry{entry}")
    }
    fn read(&self, entry: &u8, _: &str) -> SpoolReading {
        self.trace.borrow_mut().push(format!("read{entry}"));
        match if *entry == 0 { &self.peek } else { &self.owned } {
            Some(job) => SpoolReading::Job(Box::new(job.clone())),
            None if *entry == 0 => SpoolReading::Irregular,
            None => SpoolReading::Unusable("unreadable".into()),
        }
    }
    fn claim(&self, _: &u8) -> Option<u8> {
        self.trace.borrow_mut().push("claim".into());
        self.claim.then_some(1)
    }
    fn release(&self, _: &u8) -> Result<(), String> {
        self.trace.borrow_mut().push("release".into());
        self.release.clone()
    }
    fn marker_exists(&self, _: &Job) -> bool {
        false
    }
    fn hand_back(&self, job: &Job) -> Result<bool, String> {
        self.trace
            .borrow_mut()
            .push(format!("publish:{}:{}", job.due, job.args[0]));
        self.hand_back.clone()
    }
    fn heartbeat(&self, now: u64) {
        self.trace.borrow_mut().push(format!("heartbeat:{now}"));
    }
}
impl JobChildren for Children {
    fn reap(&mut self) {
        self.trace.borrow_mut().push("reap".into());
    }
    fn running(&self, _: &str) -> bool {
        self.running
    }
    fn start(&mut self, job: &Job) -> Result<(), String> {
        self.trace
            .borrow_mut()
            .push(format!("start:{}", job.args[0]));
        if self.fail {
            Err("spawn refused".into())
        } else {
            Ok(())
        }
    }
}
