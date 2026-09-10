//! Everything concrete, organized by the capability it provides.
//!
//! This crate is responsible for implementing the ports `posture-application`
//! declares, one module per real capability: the results and snapshot logs,
//! the single-record state files published by rename, the two kernel locks,
//! the known-good manifest reader with its trust check, the deployed-state
//! reader that refuses symlinks before hashing, the allowlist and controls
//! codecs, the upgrade-record reader, the bounded process runners for every
//! external tool the roster permits, the pns producer that submits through
//! pns's protocol crate, the digest record codec in posture-protocol, the
//! independent local alarm for engine submission and integrity/health failures,
//! the read-only pns ledger health check with a bounded busy timeout, the
//! privileged install and osqueryctl calls the converge alone may make, the
//! staging tree's symlink walk and private copy, the chezmoi publisher, and
//! the clock.
//!
//! Every spawned child runs under an explicit deadline with process-group
//! termination, through one command runner with a scripted double, so no
//! adapter test runs a real `sudo`, `osqueryctl`, `osqueryi`, `codesign`,
//! `tailscale` or `pns`. Enrichment and allowlist curation use these boundaries.

mod codesign;
mod staging;
pub use staging::{DesiredStaging, StagedTree};
mod command;
mod metadata;
pub use codesign::SystemInspection;
pub use command::{CommandIo, CommandOutput, CommandRunner, SystemRunner};

mod locks;
pub use locks::AllowlistWriteLock;

mod allowlist_file;

mod publisher;
pub use publisher::AllowlistPublisher;

mod allowlist_projection;

mod allowlist_read;
pub use allowlist_read::AllowlistText;

mod judge_batch;
pub use judge_batch::{BatchJudge, Collaborators, OwnedSigning, OwnedTriage};

pub use allowlist_file::AllowlistFile;

mod launchd_table;
pub use launchd_table::SystemLaunchdTable;

mod legacy_json;
mod snapshots_log;
pub use snapshots_log::SnapshotsFile;

mod clock;
pub use clock::SystemClock;

mod live_tree;
pub use live_tree::InstalledTree;

mod controls_file;
pub use controls_file::read_controls;

mod probes;
pub use probes::ControlProbes;
mod osqueryi;
pub use osqueryi::{PostureQuery, PostureTrio};

mod pns_producer;
pub use pns_producer::PnsProducer;
mod last_resort_banner;
pub use last_resort_banner::LastResortBanner;

mod state_files;
pub use state_files::PollStateFiles;

mod digest_spool;
pub use digest_spool::{DigestSpoolFile, prepare_spool_directory};

mod digest_appender;
pub use digest_appender::DigestAppendFile;

mod results_log;
pub use results_log::ResultsFile;

mod results_row;
pub use results_row::{ResultsRow, rows};

mod results_columns;

mod results_cursor;
pub use results_cursor::{CursorFile, SingleRunLock};

mod converge;
pub use converge::{
    CommandRefusal, ConvergeInstaller, OsqueryParents, OsqueryRestart, RestartTimer,
    resolve_osqueryctl,
};
