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
    /// Stop one named child and wait a bounded while for it to go.
    ///
    /// FOR THE LONG-LIVED ONES ONLY. A delivery in flight is orphaned when the
    /// daemon stops, which costs at worst one extra card; a listener orphaned
    /// the same way holds its port against every daemon that follows.
    fn terminate(&mut self, id: &str);
}

pub enum DaemonNotice {
    Output(String),
    Error(String),
}

/// What one config read says about one of the daemon's own polls.
///
/// THREE ANSWERS, NOT TWO. A config that loads and says nothing is a feature
/// that is off, and the poll is cancelled. A config that cannot be read says
/// nothing about the feature at all, and the poll already registered keeps
/// running on its last known interval: a daemon that outlives the loader its
/// config was written for must not drop its own sensors on every sweep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollSetting {
    /// The feature is on, at this many seconds between polls.
    Every(u64),
    /// The config loaded and the feature is off, absent or refused.
    Off,
    /// The config could not be read.
    Unreadable,
}

pub trait DaemonSettings {
    fn enabled(&self) -> Result<bool, String>;
    fn presence_interval(&self) -> PollSetting;
    /// How often the GitHub poll runs. It is the interval the SERVER last
    /// asked for, so this reads the poll's own state as well as the config.
    fn github_interval(&self) -> PollSetting;
    /// How often the calendar poll runs; `Off` for a feature that is off,
    /// unconfigured or refused.
    fn calendar_interval(&self) -> PollSetting;
}
