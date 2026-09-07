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
pub use ports::devices::{Bridge, Router};
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

pub use ports::nag::{NagRecords, NagSchedule};

mod poll_presence;
pub use poll_presence::{PollClaim, Polled, PresencePoll, poll_presence};
