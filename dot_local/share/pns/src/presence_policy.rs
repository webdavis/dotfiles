//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! THE NARROWING MOVED to `pns-domain`. What is left here is the path this
//! package's callers already name, including the `Snapshot` and `Full`
//! vocabulary the narrowing re-exports from the room arbitration beside it.

pub use pns_domain::presence::narrowing::{Full, Narrowing, Snapshot, narrow};
