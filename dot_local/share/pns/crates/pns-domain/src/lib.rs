//! PNS policy, expressed as total functions of their arguments.
//!
//! This crate is responsible for what PNS DECIDES: the producer-neutral
//! notification and event types, the normalized signal, the delivery plan,
//! surface, presence and visibility arbitration, quiet and dim windows, pulse
//! and lighting precedence, home-probe identity and staleness, missed
//! notification replay, recap timeline and budget, nag cadence, job
//! scheduling, and the value types that make an invalid combination
//! unrepresentable.
//!
//! It is responsible for none of what PNS TOUCHES. No filesystem, SQLite,
//! TOML, JSON, HTTP, environment variable, spawned process, macOS API, Hue or
//! UniFi call, channel discovery or command-line output appears here or in
//! anything it depends on.
//!
//! The policy arrives one behavior at a time from the legacy package at the
//! workspace root, and each move is verified against the recorded test-name
//! set in `docs/test-baseline.md`. The root package re-exports what has landed
//! here, so every existing caller keeps its old path until the composition
//! step removes the re-exports.

pub mod count;
mod decision;
mod decision_record;
mod elapsed;
pub mod home;
pub mod jobs;
pub mod lamps;
pub mod lights;
pub mod missed;
pub mod nag;
mod notification;
mod presence;
pub mod pulse;
pub mod quiet;
pub mod recap;
pub mod registry;
pub mod render;
pub mod routing;
pub mod safety;
pub mod surface;

pub use decision::{
    DEFAULT_DESK_IDLE_SECS, Decision, DecisionRequest, EnvironmentSnapshot, GateInputs, Overrides,
    SilencePolicy, SurfaceReading, decide, surface_reading,
};
pub use presence::{
    Edge, Full, Narrowing, PresenceStatus, RawPresence, Snapshot, Unreadable, chosen, classify,
    idle_secs_from_ns, narrow, unreadable_said,
};

pub use decision_record::Record;
pub use decision_record::{ABSENT, KEPT, count, printable, tri, verdict, verdicts, yes_no};
pub use routing::Delivery;

pub use elapsed::elapsed_event;
pub use notification::{Event, EventArgs};

mod focus;
pub use focus::silenced as focus_silenced;

mod presence_decision;
pub use presence_decision::PresenceDecision;

pub mod doctor;

mod setup;
pub use setup::{
    Answers, answered as setup_answer, hue_is_armed, list as setup_list,
    means_yes as setup_affirmed, router_backend as setup_router_backend, router_is_armed,
};

mod condenser;
pub use condenser::{condenser_prompt, condenser_verdict};

pub mod retry;
