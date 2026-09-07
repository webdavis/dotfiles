use uu_application::{LockFailure, MarkerSnapshot, RunState, StateWriteFailure, StreakSnapshot};

use crate::state;

pub struct FileRunState<'a>(pub &'a str);

impl RunState for FileRunState<'_> {
    type Guard = state::RunLock;

    fn acquire(&self) -> Result<Self::Guard, LockFailure> {
        state::acquire_lock(self.0)
    }

    fn prune_removed_lanes(&self, declared: &[&str]) {
        state::prune_removed_lanes(self.0, declared);
    }

    fn marker(&self) -> MarkerSnapshot {
        let path = state::marker_path(self.0);
        MarkerSnapshot {
            value: state::read_marker(&path),
            location: path.display().to_string(),
        }
    }

    fn write_marker(&self, epoch: i64) -> Result<(), StateWriteFailure> {
        let path = state::marker_path(self.0);
        state::write_marker(&path, epoch).map_err(|error| StateWriteFailure {
            location: path.display().to_string(),
            cause: error.to_string(),
        })
    }

    fn streak(&self, lane: &str) -> StreakSnapshot {
        let path = state::streak_path(self.0, lane);
        StreakSnapshot {
            value: state::read_streak(&path),
            location: path.display().to_string(),
        }
    }

    fn write_streak(&self, lane: &str, value: u32) -> Result<(), StateWriteFailure> {
        let path = state::streak_path(self.0, lane);
        state::write_streak(&path, value).map_err(|cause| StateWriteFailure {
            location: path.display().to_string(),
            cause,
        })
    }
}
