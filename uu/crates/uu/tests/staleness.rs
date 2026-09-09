//! The staleness bound: a lane that has gone quiet says so, exactly once per streak.

mod support;

use support::*;

#[path = "staleness/failures.rs"]
mod failures;
#[path = "staleness/streaks.rs"]
mod streaks;
