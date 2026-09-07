use uu_domain::Marker;

/// Why `acquire` could not hand back a lock: the ONE arm that is genuine
/// contention, and everything else. The call site says something different
/// for each, because "to avoid racing the run that already holds it" is only
/// true for `Contended`: a directory that could not be created or a lock file
/// that could not even be opened is an environment problem with its own real
/// cause, and this is the one place the operator hears about it, so blaming a
/// race that never happened would send them chasing the wrong thing.
#[derive(Debug, PartialEq, Eq)]
pub enum LockFailure {
    /// `flock` itself refused: another run genuinely holds the lock right
    /// now.
    Contended(String),
    /// The lock file, or the directory it lives in, could not even be
    /// opened.
    Unavailable(String),
}

pub struct MarkerSnapshot {
    pub value: Marker,
    pub location: String,
}

/// What reading a lane's streak file found.
///
/// `Absent` COVERS ONLY `NotFound`: that is the one case that legitimately
/// means a fresh lane, or one that has never had a non-success run. Anything
/// else the file could say (unreadable, a directory sitting where the file
/// belongs, content that is not a plain count) is `Unreadable`, never a
/// silent zero: zero would forgive whatever streak the file actually held,
/// which is the fail-open this whole capability exists to refuse.
#[derive(Debug, PartialEq, Eq)]
pub enum Streak {
    Absent,
    Value(u32),
    Unreadable(String),
}

pub struct StreakSnapshot {
    pub value: Streak,
    pub location: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StateWriteFailure {
    pub location: String,
    pub cause: String,
}

pub trait RunState {
    type Guard;

    fn acquire(&self) -> Result<Self::Guard, LockFailure>;
    fn prune_removed_lanes(&self, declared: &[&str]);
    fn marker(&self) -> MarkerSnapshot;
    fn write_marker(&self, epoch: i64) -> Result<(), StateWriteFailure>;
    fn streak(&self, lane: &str) -> StreakSnapshot;
    fn write_streak(&self, lane: &str, value: u32) -> Result<(), StateWriteFailure>;
}
