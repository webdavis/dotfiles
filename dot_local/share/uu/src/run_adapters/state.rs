use uu_application::{LockFailure, MarkerSnapshot, RunState, StateWriteFailure, StreakSnapshot};

use crate::state::{lock, marker, streak};

pub struct FileRunState<'a>(pub &'a str);

impl RunState for FileRunState<'_> {
    type Guard = lock::RunLock;

    fn acquire(&self) -> Result<Self::Guard, LockFailure> {
        lock::acquire(self.0)
    }

    fn prune_removed_lanes(&self, declared: &[&str]) {
        streak::prune_removed_lanes(self.0, declared);
    }

    fn marker(&self) -> MarkerSnapshot {
        let path = marker::path(self.0);
        MarkerSnapshot {
            value: marker::read(&path),
            location: path.display().to_string(),
        }
    }

    fn write_marker(&self, epoch: i64) -> Result<(), StateWriteFailure> {
        let path = marker::path(self.0);
        marker::write(&path, epoch).map_err(|error| StateWriteFailure {
            location: path.display().to_string(),
            cause: error.to_string(),
        })
    }

    fn streak(&self, lane: &str) -> StreakSnapshot {
        let path = streak::path(self.0, lane);
        StreakSnapshot {
            value: streak::read(&path),
            location: path.display().to_string(),
        }
    }

    fn write_streak(&self, lane: &str, value: u32) -> Result<(), StateWriteFailure> {
        let path = streak::path(self.0, lane);
        streak::write(&path, value).map_err(|cause| StateWriteFailure {
            location: path.display().to_string(),
            cause,
        })
    }
}
