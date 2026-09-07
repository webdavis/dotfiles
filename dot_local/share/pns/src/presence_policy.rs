//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! THE NARROWING MOVED to `pns-domain`. What is left here is the path this
//! package's callers already name. `Snapshot` and `Full` remain shared with
//! the room arbitration beside the narrowing.

pub use pns_domain::{Full, Narrowing, Snapshot, narrow};
