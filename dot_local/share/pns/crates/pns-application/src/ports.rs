//! The capabilities the use cases need, each declared as a trait HERE and
//! implemented outside.
//!
//! ONE TRAIT PER READING OR PER WRITE, deliberately narrow, so a test
//! substitutes exactly the capability it is about and a use case never grows a
//! path that touches the outside world. Nothing in this module constructs an
//! HTTP client, spawns a process, opens a file or reads an environment
//! variable; the adapters do that, and they depend on this crate rather than
//! the other way round.

pub mod clock;
pub mod delivery;
pub mod devices;
pub mod environment;
pub mod harness;
pub mod notification;
pub mod process;
pub mod records;
