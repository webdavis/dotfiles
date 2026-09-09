use crate::*;
use pns_domain::{
    Narrowing, Snapshot,
    jobs::Job,
    lamps::{Inventory, config::Lights},
    lights::{phase::HeldEntry, streak::Streak, unread::News},
};
use std::cell::RefCell;

#[derive(Default)]
pub(super) struct World {
    pub log: RefCell<Vec<String>>,
    pub held: RefCell<Option<Vec<HeldEntry>>>,
    pub shell: Option<u64>,
    pub complaint: RefCell<String>,
}
impl World {
    pub fn empty() -> Self {
        Self {
            held: RefCell::new(Some(Vec::new())),
            ..Self::default()
        }
    }
    pub fn note(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    pub fn run(&self, lights: Option<&Lights>, now: Option<u64>, available: bool) {
        MaintainLamps {
            records: self,
            work: self,
            markers: self,
            claim: self,
            jobs: self,
        }
        .run(
            lights,
            &|| {
                self.note("clock");
                now
            },
            |refresh| {
                self.note(format!("connect({refresh:?})"));
                available.then_some(self)
            },
            LampReadings {
                minutes: |_| {
                    self.note("minutes");
                    Some(720)
                },
                presence: || {
                    self.note("presence");
                    None
                },
                last_interaction: || {
                    self.note("interaction");
                    Some(100)
                },
                interval: || {
                    self.note("interval");
                    (|| 0, |_| {})
                },
            },
            |line| self.note(format!("said({line})")),
        );
    }
}
impl AgentWork for World {
    fn statuses(&self) -> Vec<String> {
        self.note("work");
        Vec::new()
    }
}
impl LampMarkers for World {
    fn sweep_legacy(&self) {
        self.note("legacy");
    }
    fn shell_since(&self) -> Option<u64> {
        self.note("shell");
        self.shell
    }
    fn leases(&self, _: u64, _: u64) -> Vec<u64> {
        self.note("leases");
        Vec::new()
    }
    fn blocked(&self, _: u64, _: u64) -> Vec<u64> {
        self.note("blocked");
        Vec::new()
    }
}
impl LampHouseRecords for World {
    fn advance_streak(&self, working: bool, _: u64) -> Option<Streak> {
        self.note(format!("streak({working})"));
        None
    }
    fn news(&self) -> News {
        self.note("news");
        News::default()
    }
}
impl LampMutes for World {
    fn read(&self) -> (Vec<pns_domain::lights::mute::Muted>, Vec<String>) {
        self.note("mutes");
        (Vec::new(), Vec::new())
    }
    fn write(&self, _: &[pns_domain::lights::mute::Muted]) -> Result<(), String> {
        panic!("tick never writes mutes")
    }
}
impl HeldLamps for World {
    fn read(&self) -> Option<Vec<HeldEntry>> {
        self.note("held");
        self.held.borrow().clone()
    }
    fn remember(&self, entries: &[HeldEntry]) -> Result<(), String> {
        self.note("remember-held");
        *self.held.borrow_mut() = Some(entries.to_vec());
        Ok(())
    }
}
impl LampComplaints for World {
    fn remembered(&self, kind: LampComplaint) -> String {
        assert!(matches!(kind, LampComplaint::Tick));
        self.complaint.borrow().clone()
    }
    fn remember(&self, _: LampComplaint, text: Option<&str>) {
        self.note("complaint-memory");
        *self.complaint.borrow_mut() = text.unwrap_or("").into();
    }
}
impl PresenceDecisions for World {
    fn record(&self, _: &Snapshot, _: &Narrowing) {
        self.note("narrowed");
    }
}
impl LampTickClaim for World {
    type Guard = ();
    fn claim(&self, _: u64) -> Option<()> {
        self.note("claim");
        Some(())
    }
}
impl JobSpool for World {
    fn pending(&self, _: &str) -> Option<Job> {
        None
    }
    fn schedule(&self, job: &Job, now: u64) -> Result<(), String> {
        self.note(format!("schedule({},{},{now})", job.due, job.until));
        Ok(())
    }
    fn cancel(&self, _: &str) -> Result<bool, String> {
        panic!("no cancellation")
    }
}
impl LampBridge for &World {
    fn inventory(&self) -> Option<Inventory> {
        self.note("inventory");
        Some(Inventory::default())
    }
    fn write(&self, path: &str, _: &LampWrite) {
        self.note(format!("write({path})"));
    }
}
