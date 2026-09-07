//! uu: the unattended-upgrades tool. One binary, one lane per thing that
//! updates itself, one record per run.
//!
//! Concrete configuration, state, process, clock and delivery adapters.
//! Domain policy, run sequencing and protocol encodings keep their own crates.
//! Process boundaries use the existing traits so tests exercise uu behavior
//! against owned fixtures.
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

mod alert;
mod config;
mod deadline;
mod lanes;
mod record;
mod registration;
mod schedule;

mod delivery;
mod run_adapters;
mod runner;
mod state;
mod system;
mod watchdog;

pub use config::{
    BrewLane, CommandLane, Config, ConfigError, HerdrLane, LoadOutcome, NpmLane, NvimPluginsLane,
    UvLane, config_path, load_config,
};
pub use delivery::EngineRunDelivery;
pub use record::gap_line;
pub use run_adapters::{
    ConfiguredLaneExecutor, ConsoleRunPresentation, FileRunState, SystemRunClock,
};
pub use schedule::{DEFAULT_LABEL, render_plist};
pub use state::{marker_path, read_marker};
pub use system::{home, now_epoch, resolve};

pub use lanes::{CommandRunner, LaneAdapter, Ran, Verdict};
pub use registration::LaneRegistration;
