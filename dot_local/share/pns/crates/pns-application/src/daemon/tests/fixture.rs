use crate::*;
use pns_domain::jobs::Job;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub(super) struct World {
    pub log: Rc<RefCell<Vec<String>>>,
    pub settings: Vec<Result<bool, String>>,
    pub reads: Rc<Cell<usize>>,
    pub now: Option<u64>,
    pub pending: Rc<RefCell<Option<Job>>>,
}
impl World {
    pub fn new(settings: Vec<Result<bool, String>>) -> Self {
        Self {
            log: Rc::default(),
            settings,
            reads: Rc::default(),
            now: Some(100),
            pending: Rc::default(),
        }
    }
    pub fn run(&self, prepare: bool) -> i32 {
        self.run_with_retry(prepare, |_| Ok(()))
    }
    pub fn run_with_retry(
        &self,
        prepare: bool,
        mut retry: impl FnMut(u64) -> Result<(), String>,
    ) -> i32 {
        RunDaemon {
            settings: self,
            clock: self,
        }
        .run(
            || {
                self.log.borrow_mut().push("prepare".into());
                if !prepare {
                    return Err("spool refused".into());
                }
                Ok((self.clone(), self.clone(), || {
                    assert!(
                        self.count("sleep") < 61,
                        "daemon exceeded the bounded fixture ticks"
                    );
                    self.log.borrow_mut().push("sleep".into())
                }))
            },
            |now, _| retry(now),
            |notice| {
                self.log.borrow_mut().push(match notice {
                    DaemonNotice::Output(line) => format!("out:{line}"),
                    DaemonNotice::Error(line) => format!("err:{line}"),
                })
            },
        )
    }
    pub fn count(&self, text: &str) -> usize {
        self.log
            .borrow()
            .iter()
            .filter(|s| s.as_str() == text)
            .count()
    }
}
impl DaemonSettings for World {
    fn enabled(&self) -> Result<bool, String> {
        self.log.borrow_mut().push("settings".into());
        let at = self.reads.get();
        self.reads.set(at + 1);
        self.settings
            .get(at)
            .expect("loop exceeded its bounded settings sequence")
            .clone()
    }
    fn presence_interval(&self) -> Option<u64> {
        self.log.borrow_mut().push("presence".into());
        Some(7)
    }
}
impl Clock for World {
    fn now_secs(&self) -> Option<u64> {
        self.log.borrow_mut().push("clock".into());
        self.now
    }
}
impl JobSpool for World {
    fn pending(&self, _: &str) -> Option<Job> {
        self.pending.borrow().clone()
    }
    fn schedule(&self, job: &Job, now: u64) -> Result<(), String> {
        self.log.borrow_mut().push(format!("schedule({now})"));
        *self.pending.borrow_mut() = Some(job.clone());
        Ok(())
    }
    fn cancel(&self, id: &str) -> Result<bool, String> {
        self.log.borrow_mut().push(format!("cancel({id})"));
        Ok(self.pending.borrow_mut().take().is_some())
    }
}
impl JobChildren for World {
    fn reap(&mut self) {
        self.log.borrow_mut().push("reap".into());
    }
    fn running(&self, _: &str) -> bool {
        panic!("empty spool");
    }
    fn start(&mut self, _: &Job) -> Result<(), String> {
        panic!("empty spool");
    }
}
impl DaemonSpool for World {
    type Entry = u8;
    fn entries(&self) -> Vec<u8> {
        self.log.borrow_mut().push("entries".into());
        Vec::new()
    }
    fn id(&self, _: &u8) -> Option<String> {
        panic!("empty spool");
    }
    fn describe(&self, _: &u8) -> String {
        panic!("empty spool");
    }
    fn read(&self, _: &u8, _: &str) -> SpoolReading {
        panic!("empty spool");
    }
    fn claim(&self, _: &u8) -> Option<u8> {
        panic!("empty spool");
    }
    fn release(&self, _: &u8) -> Result<(), String> {
        panic!("empty spool");
    }
    fn marker_exists(&self, _: &Job) -> bool {
        panic!("empty spool");
    }
    fn hand_back(&self, _: &Job) -> Result<bool, String> {
        panic!("empty spool");
    }
    fn heartbeat(&self, _: u64) {
        self.log.borrow_mut().push("heartbeat".into());
    }
}
