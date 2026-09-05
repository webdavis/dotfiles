//! The routing policy moved to `pns-domain`; this module re-exports it and
//! keeps its tests, which drive a real parsed config the domain crate cannot
//! take. Both follow when the config edge moves.

pub use pns_domain::routing::{Leg, ReportMode, channel_plan};

#[cfg(test)]
mod tests;
