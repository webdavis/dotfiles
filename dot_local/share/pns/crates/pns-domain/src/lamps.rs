//! The lamp policy: what a lamp can say, and how a reading resolves to one.

pub mod config;
mod dim;
mod inventory;
mod mute;
mod resolve;
mod window;

pub use dim::{DimWindow, Showing, dim_showing};
pub use inventory::{Fixture, Inventory, Lamp, Missing, Unresolved, missing_sentence};
pub use mute::{Muting, mutable_names, muted_now};
pub use resolve::{LEVELS, Routed, Routing, remember, resolve};
pub use window::{QuietWindow, parse_window, quiet_now};

mod deadline;
pub use deadline::tick_bridge_deadline;

/// The spool name the tick job is registered under. ONE JOB FOR THE WHOLE
/// HOUSE, not one per lamp: the tick derives every state from scratch and
/// writes every fixture, so a second job would be a second writer of the same
/// bulbs.
pub const LIGHTS_JOB: &str = "lights";

mod path;
mod reading;
mod render;
pub use path::fixture_path;
pub use reading::Reading;
pub use render::{held_render, pulse_render};
