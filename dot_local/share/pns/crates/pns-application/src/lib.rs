//! The PNS use cases, and the ports they own.
//!
//! This crate is responsible for orchestrating one operator-meaningful
//! operation at a time (submitting a notification, requesting an approval,
//! replaying missed notifications, building a return recap, running a nag,
//! reading the home probe, reconciling the lights, taking a loop lease,
//! running a daemon tick, scheduling or cancelling a job, running the doctor,
//! running setup) by combining pns-domain policy with capabilities it declares
//! and does not implement.
//!
//! Those declarations are the point. A use case owns the trait it consumes, so
//! the dependency runs inward: adapters implement these ports, and this crate
//! never names an adapter. It constructs no HTTP client, spawns no process,
//! opens no file and reads no environment variable.
//!
//! The ports are declared first, in `ports`, because every use case that
//! follows is written against them. Three of the readings have moved in
//! already: the environment probes, the surface and visibility they are read
//! into, and the plugin selection.

mod environment_reading;
mod ports;
mod replay_missed;
mod request_approval;
mod selection;
mod submit_notification;

pub use environment_reading::{decide, operator_surface};

pub use ports::clock::Clock;
pub use ports::delivery::{
    ApprovalForwarder, LampSignal, MissedReplay, NotificationDestination, RecapPublisher,
    ReplayDelivery,
};
pub use ports::devices::{Router, StalenessMemory};
pub use ports::environment::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
};
pub use ports::harness::HarnessPayload;
pub use ports::notification::{PhoneSuppression, RaiseNotification};
pub use ports::process::CommandRunner;
pub use ports::records::{
    ActivityRing, BlockedMarker, Claim, DecisionRing, JobSpool, Journal, LampRecords, LightsTick,
    LoopLease, ReturnMoment,
};
pub use replay_missed::{RecapPolicy, ReplayMissedNotifications};
pub use request_approval::RequestApproval;
pub use selection::{ConfigOutcome, select_plugins};
pub use submit_notification::{Attempt, Submission, SubmitNotification};

pub use ports::nag::{Claimed, NagRecords, NagSchedule};

mod poll_presence;
pub use poll_presence::{PollClaim, Polled, PresencePoll, poll_presence};

mod run_nag;
pub use run_nag::{Outcome as NagOutcome, RunNag};

mod read_home_probe;
pub use read_home_probe::{ReadHomeProbe, read_home};

mod arm_nag;
pub use arm_nag::ArmNag;
mod clear_nag;
pub use clear_nag::clear_nag;

mod run_daemon_tick;
pub use ports::jobs::{DaemonNotice, DaemonSpool, JobChildren, SpoolReading};
pub use run_daemon_tick::RunDaemonTick;

mod schedule_job;
pub use schedule_job::{ScheduleJob, Until, cancel_job};

pub use ports::lights::{LampMutes, LoopLeases};
mod set_lights_quiet;
pub use set_lights_quiet::{SetLightsQuiet, ad_hoc_quiet, quiet_names};

mod loop_lease;
pub use loop_lease::AcquireLoopLease;

pub use ports::lamps::{HeldLamps, LampBridge, LampTickClaim, LampWrite, PresenceDecisions};

mod lamp_breath;
mod lamp_narrowing;
mod lamp_routing;
mod reconcile_lights;
mod signal_lamps;
pub use lamp_breath::{Breathing, drive_breaths};
pub use reconcile_lights::{ReconcileLights, TickReading};
pub use signal_lamps::SignalLamps;

mod clear_lamps;
mod lamp_registration;
pub use clear_lamps::clear_held_lamps;
pub use lamp_registration::{ORDINARY_LEASE_SECS, register_lights_tick, schedule_lights_tick};

mod lamp_house;
mod lamp_interaction;
pub use lamp_house::{ReadLampHouse, Standing};
pub use lamp_interaction::last_lamp_interaction;
pub use ports::lamp_house::{AgentWork, LampHouseRecords, LampMarkers};

mod lamp_complaints;
mod lamp_signal_gate;
pub use lamp_complaints::report_lamp_complaints;
pub use lamp_signal_gate::{signal_after_delivery, signal_mapped};
pub use ports::lights::{LampComplaint, LampComplaints};

mod maintain_lamps;
pub use maintain_lamps::{LampReadings, MaintainLamps};

mod build_return_recap;
mod post_return_recap;
pub use build_return_recap::{BuildReturnRecap, RECAP_USAGE, recap_bounds, recap_wall_clock};
pub use ports::recap::{Fetched, MergedPullRequestSource, ReviewNoteSource, Summarizer};
pub use post_return_recap::post_return_recap;

mod presence_registration;
mod run_daemon;
pub use ports::jobs::DaemonSettings;
pub use presence_registration::{PRESENCE_DAEMON_FLAG, ensure_presence_poll};
pub use run_daemon::{RunDaemon, daemon_tick};

mod run_doctor;
pub use run_doctor::{DOCTOR_OPENING, DoctorActions, RunDoctor, doctor_pulse};

pub use run_doctor::{DoctorBridge, FocusReading, doctor_focus, doctor_lamps};
mod delivery_panic;
pub use delivery_panic::deliver_guarded;

pub use ports::setup::{ConfigPublisher, ConfigRenderer, Terminal};
mod run_setup;
pub use run_setup::{RunSetup, SETUP_USAGE};
