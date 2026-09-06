//! Whether the operator is home, read off the router.
//!
//! POLICY ONLY: no network, no file, no clock. The composition root asks the
//! router and hands the answer in.

pub mod identity;
pub mod reading;
pub mod staleness;
