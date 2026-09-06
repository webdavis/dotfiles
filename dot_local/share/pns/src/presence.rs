//! Where the operator is: the raw readings turned into the units the
//! arbitration compares.
//!
//! THE POLICY MOVED to `pns-domain`, where it sits beside the room
//! arbitration and the narrowing that read its answers. What is left here is
//! the path this package's callers already name.

pub use pns_domain::presence::status::{
    PresenceStatus, Unreadable, classify, idle_secs_from_ns, unreadable_said,
};
