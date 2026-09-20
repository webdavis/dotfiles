//! Everything concrete, organized by the capability it provides.
//!
//! This crate is responsible for implementing the ports `posture-application`
//! declares, one module per real capability: the results and snapshot logs,
//! the single-record state files published by rename, the two kernel locks,
//! the known-good manifest reader with its trust check, the deployed-state
//! reader that refuses symlinks before hashing, the allowlist and controls
//! codecs, the upgrade-record reader, the bounded process runners for every
//! external tool the roster permits, the two delivery sinks (a signed POST to
//! a hermes webhook route and a page handed to a configured producer command),
//! the digest record codec in posture-protocol, the
//! independent local alarm for engine submission and integrity/health failures,
//! the read-only engine ledger health check with a bounded busy timeout, the
//! privileged install and osqueryctl calls the converge alone may make, the
//! staging tree's symlink walk and private copy, the chezmoi publisher, and
//! the clock.
//!
//! Every spawned child runs under an explicit deadline with process-group
//! termination, through one command runner with a scripted double, so no
//! adapter test runs a real `sudo`, `osqueryctl`, `osqueryi`, `codesign`,
//! `tailscale` or a producer command. Enrichment and allowlist curation use
//! these boundaries.

mod codesign;
mod private_directory;
mod staging;
pub use staging::{DesiredStaging, StagedTree};
mod command;
mod metadata;
pub use codesign::SystemInspection;
pub use command::{CommandIo, CommandOutput, CommandRunner, SystemRunner, is_executable};
mod command_duration;
pub use command_duration::{COMMAND_DURATION_CEILING, parse_command_duration};

mod locks;
pub use locks::AllowlistWriteLock;

mod allowlist_file;

mod publisher;
pub use publisher::AllowlistPublisher;

mod allowlist_projection;

mod allowlist_read;
pub use allowlist_read::AllowlistText;

mod known_good_read;
pub use known_good_read::KnownGoodManifests;

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
mod process_lookup;
pub use process_lookup::{LibprocProcesses, ProcessLookup, WALK_DEADLINE};
mod osqueryi;
pub use osqueryi::{PostureQuery, PostureTrio};

// The producer API's two documents, spoken by the producer path alone. Nothing
// outside this crate names them: an engine is reached through `alert_sink`.
mod producer;
mod wire;
pub use producer::ProducerCommand;
mod hermes;
mod request_id;
mod sink;
pub use hermes::{CriticalCopy, HermesWebhook};
mod signed_post;
pub use signed_post::{PostOutcome, SignedPost, UreqSignedPost, delivered, sign};
mod notify;
pub use notify::{
    DEFAULT_WEBHOOK_BASE, JobSettings, Notify, NotifyMode, agent_labels, alert_sink, config_path,
    job_settings,
};
mod banner_only;
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
    resolve_osqueryctl, resolve_osqueryi,
};

mod funnel_status;
pub use funnel_status::read_funnel;
mod funnel_state;
pub use funnel_state::FunnelStateFile;
mod watchdog_processes;
pub use watchdog_processes::SystemWatchdogProcesses;
mod watchdog_state;
pub use watchdog_state::WatchdogStateFile;
mod watchdog_queue;
pub use watchdog_queue::QueueDatabase;
mod gateway_health;
pub use gateway_health::GatewayProbe;
mod watchdog_audit;
pub use watchdog_audit::WatchdogAudit;

mod integrity_triage;
pub use integrity_triage::file_integrity_triage;

mod sshd_tree;
pub use sshd_tree::SshConfigTree;

mod ssh_commands;
mod ssh_signals;
pub use ssh_commands::{SshFileInstaller, SshKeyscan, SshLaunchd, SshdCommand};
pub use ssh_signals::{SshSignals, ssh_install_cancelled};
mod ssh_user;
pub use ssh_user::{current_uid, current_user_name};

#[cfg(test)]
mod test_gateway;
#[cfg(test)]
mod test_processes;
#[cfg(test)]
mod test_sandbox;
