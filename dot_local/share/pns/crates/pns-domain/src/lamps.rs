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
