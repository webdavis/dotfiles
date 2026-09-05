//! The lamp policy: what a lamp can say, and how a reading resolves to one.
//!
//! Only the closed set of lamp behaviours has moved so far, because the pulse
//! policy beside it answers in that type and cannot reach back into the legacy
//! package. The rest of the lamp policy follows.

pub mod config;
