//! The pipeline's policy, expressed as total functions of their arguments.
//!
//! This crate is responsible for what posture DECIDES: which result-log rows
//! are findings and how severe they are, the page-or-digest-or-log gate, the
//! allowlist and known-good verdicts, the tamper decision over a deployed
//! state and a manifest answer, the upgrade-record triage, the page and digest
//! vocabulary with their caps, the canary freshness rule, the watchdog's five
//! probes and the manifest audit, the controls file's validations, the
//! poller's classifiers and baseline trust rules, the funnel exposure
//! transitions, the converge drift verdicts and restart evidence, the cursor
//! grammar, and the signing classification.
//!
//! It is responsible for none of what posture TOUCHES. No filesystem, clock,
//! JSON, environment variable, spawned process, privilege, macOS API or
//! command-line output appears here or in anything it depends on. Every
//! reading it judges arrives as a typed value with its own "could not read"
//! state, never as an empty string, so each fail-safe direction is an enum
//! arm rather than an emptiness check.
//!
//! Finding normalization policy lives here; wire values remain in adapters.

mod finding;

mod converge_policy;
pub use converge_policy::{
    CommandTrustRefusal, ConvergeDirectory, ConvergeFile, ParentPid, RestartBounds, command_trust,
};

pub use finding::{Detector, EnrichmentPaths};

mod cursor;
mod digest;
mod gate;
mod page;
mod sanitize;
mod severity;

pub use cursor::{
    Advance, LiveLog, StoredCursor, advance, parse as parse_cursor, render as render_cursor,
};
pub use digest::{BULLETS_PER_GROUP, DigestEntry, GROUP_LIMIT, render_digest};
pub use gate::{
    FileCategory, GateColumns, GateEvidence, GateFinding, GateOutcome, IntegrityVerdict,
    LaunchdIdentity, Signing, Triage, gate,
};
pub use page::{BLOCK_LIMIT, BODY_LIMIT, Page, PageColumns, PageFinding, render_page};
pub use severity::{Action, ProtectionState, Severity, severity};

mod allowlist;
mod integrity;
mod known_good;

pub use allowlist::{
    Allowlist, AllowlistChange, AllowlistEntry, AllowlistLine, AllowlistVerdict, CuratedLine,
    CurationRefusal, allowlist_verdict, curate_allowlist, relativize_allowlist_identity,
    valid_allowlist_label,
};
pub use integrity::{
    DeployedState, FileKind, Rehash, deployed_state_known_good, integrity_verdict,
};
pub use known_good::{
    KnownGood, KnownGoodTuple, Manifest, ManifestAuthority, ManifestDigest, ManifestKind,
    manifest_for, manifest_trustworthy,
};

mod enrich;
pub use enrich::{CodeTrust, Enrichment, classify_signing, is_interpreter};

mod canary;
pub use canary::{CanaryEpoch, CanaryFreshness, canary_freshness};

mod heartbeat;
pub use heartbeat::{HeartbeatText, HeartbeatWindow, heartbeat_text};

mod audit;
pub use audit::{
    AuditBounds, AuditFile, AuditFinding, AuditKind, AuditManifest, AuditRefusal, AuditReport,
    AuditRow, audit_scan,
};

mod watchdog;
pub use watchdog::{
    Agent, AgentExit, AgentJudgment, AgentReading, AgentState, AuditFingerprint, AuditJudgment,
    AuditMemory, ExitCode, WatchdogPage, audit_fingerprint_input, judge_agent, judge_audit,
    osquery_problem, route_problem, state_problem, watchdog_page,
};

mod drift;
pub use drift::{
    ContentComparison, Drift, LiveAttributes, LiveEntry, directory_drift, file_drift,
    restart_required,
};

mod controls;
pub use controls::{
    Control, ControlReader, ControlRecord, ControlValue, ControlsInput, ControlsRefusal,
    ControlsRefusalKind, validate_controls,
};

mod funnel;
pub use funnel::{
    AllowFunnel, FunnelAlert, FunnelBaseline, FunnelPlan, FunnelReading, FunnelState,
    classify_funnel, funnel_baseline, plan_funnel, render_funnel_exposure,
};

mod poll;
pub use poll::{
    BaselineUpdate, ControlObservation, ControlPrior, ControlReading, ControlsRead, LuluProfile,
    PollBaseline, PollPage, PollPlan, StoredControl, Trio, TrioReading, classify_autologin,
    classify_filevault, classify_lulu_profile, classify_messages, classify_pgrep, plan_poll,
    poll_persistence_gap, trusted_poll_baseline,
};
