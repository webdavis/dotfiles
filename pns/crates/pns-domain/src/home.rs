//! Whether the operator is home, read off the router.
//!
//! POLICY ONLY: no network, no file, no clock. The composition root asks the
//! router and hands the answer in.

mod identity;
mod reading;
mod staleness;

pub use identity::{Client, DeviceIdentity, DeviceIdentityError, DeviceKey, UNIFI_TYPE};
pub use reading::{HomePresence, HomeReading, KeyOutcome, KeyReading, home_reading};
pub use staleness::{Staleness, episode_id, is_new_staleness, stale_identifiers, stale_warning};
