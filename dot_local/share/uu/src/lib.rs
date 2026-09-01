//! uu: the unattended-upgrades tool. One binary, one lane per thing that
//! updates itself, one record per run.
//!
//! THE SPLIT THAT MATTERS, borrowed from pns beside it. The decision modules
//! (`config`, `record`, `alert`, `schedule`, and the lane policy in `lanes`)
//! are total functions of their arguments: no network, no clock, no
//! environment. Every process boundary is a narrow trait with one production
//! implementation, so the thing under test is what uu DECIDES and never what
//! herdr or launchd does.
//!
//! FAILURE DIRECTIONS, declared rather than improvised:
//!
//! - Lanes FAIL OPEN as a group. One lane's failure never stops the next, and
//!   the run still ends at exit 0, because the scheduler retrying a whole week
//!   later is not a recovery and a job that hides its other lanes' work is
//!   worse than one that reports a failure.
//! - Records FAIL LOUD. The weekly entry's whole value is that its absence
//!   means something, so a records block that cannot post says so on stderr.
//! - Alerts FAIL OPEN. An absent or refusing pns engine is logged and the run
//!   stays clean: a notification must never fail the work it reports on.

pub mod alert;
pub mod config;
pub mod lanes;
pub mod record;
pub mod schedule;
