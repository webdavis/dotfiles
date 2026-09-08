//! The capabilities the use cases need, each declared as a trait HERE and
//! implemented outside.
//!
//! ONE TRAIT PER READING OR PER WRITE, deliberately narrow, so a test
//! substitutes exactly the capability it is about and a use case never grows a
//! path that touches the outside world. Nothing in this module constructs an
//! HTTP client, spawns a process, opens a file or reads an environment
//! variable; the adapters do that, and they depend on this crate rather than
//! the other way round.

pub(super) mod clock;
pub(super) mod delivery;
pub(super) mod devices;
pub(super) mod environment;
pub(super) mod harness;
pub(super) mod notification;
pub(super) mod process;
pub(super) mod records;

pub(super) mod nag;

pub(super) mod jobs;

pub(super) mod lights;

pub(super) mod lamps;

pub(super) mod lamp_house;

pub(super) mod recap;

pub(crate) mod setup;

pub(super) mod ledger;

pub(super) mod decision_outcomes;
