use pns_domain::jobs::Job;

/// Reading a schedule never claims it. Claiming preserves the entry's actual
/// identity, and only a second read of that owned entry may lead to action.
pub trait DaemonSpool {
    type Entry: Clone + Ord;
    fn entries(&self) -> Vec<Self::Entry>;
    fn id(&self, entry: &Self::Entry) -> Option<String>;
    fn describe(&self, entry: &Self::Entry) -> String;
    fn read(&self, entry: &Self::Entry, id: &str) -> SpoolReading;
    fn claim(&self, entry: &Self::Entry) -> Option<Self::Entry>;
    fn release(&self, claim: &Self::Entry) -> Result<(), String>;
    fn marker_exists(&self, job: &Job) -> bool;
    /// Create only if absent: a newer client registration wins over this claim.
    fn hand_back(&self, job: &Job) -> Result<bool, String>;
    /// Failure is quiet: a missed heartbeat costs only a doctor reading.
    fn heartbeat(&self, now: u64);
}

pub enum SpoolReading {
    Job(Box<Job>),
    Irregular,
    Unusable(String),
}

/// Children that this daemon actually owns, including their monotonic bounds.
pub trait JobChildren {
    fn reap(&mut self);
    fn running(&self, id: &str) -> bool;
    fn start(&mut self, job: &Job) -> Result<(), String>;
}

pub enum DaemonNotice {
    Output(String),
    Error(String),
}

pub trait DaemonSettings {
    fn enabled(&self) -> Result<bool, String>;
    fn presence_interval(&self) -> Option<u64>;
}
