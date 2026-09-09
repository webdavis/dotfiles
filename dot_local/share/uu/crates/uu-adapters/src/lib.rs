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
mod bootstrap;
mod config;
pub use config::RotateLogsLane;

mod deadline;
mod lanes;
mod record;
mod registration;
mod schedule;

mod adapters;
mod delivery;
mod runner;
mod state;
mod system;
mod watchdog;

pub use adapters::{ConfiguredLaneExecutor, ConsoleRunPresentation, FileRunState, SystemRunClock};
pub use config::{
    BrewLane, ClaudePluginsLane, CommandLane, Config, ConfigError, HerdrLane, LoadOutcome, NpmLane,
    NvimMasonLane, NvimParsersLane, NvimPluginsLane, NvimSmokeTestLane, UvLane, config_path,
    load_config,
};
pub use delivery::EngineRunDelivery;
pub use record::gap_line;
pub use schedule::{DEFAULT_LABEL, render_plist};
pub use state::{marker_path, read_marker};
pub use system::{home, now_epoch, resolve};

pub use lanes::{CommandRunner, LaneAdapter, Ran, Verdict};
pub use registration::LaneRegistration;

pub use bootstrap::{BootstrapLane, bootstrap_lane};

pub use config::SkillsConfig;
pub use lanes::{HermesRegistryEntry, SkillsRoster};

pub use lanes::SkillsEnvironment;
pub use lanes::{
    SkillsBuildMode, SkillsCandidate, SkillsGenerationStore, SkillsPublication, SkillsRecovery,
    exchange_skills_directories,
};

pub use lanes::SkillsForkWatch;

pub use lanes::capture_skills_updater;
