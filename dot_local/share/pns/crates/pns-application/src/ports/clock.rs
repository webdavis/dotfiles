//! The wall clock.

/// The wall clock, in whole seconds since the epoch.
///
/// ONE READ PER OPERATION, and that is the whole reason this is a port rather
/// than a call to the standard library at the point of use. Every age in one
/// decision is measured against ONE moment; two reads of the clock inside one
/// event is what drifted a phone reading and a desk reading apart, so a use
/// case takes the moment once and carries it.
///
/// `None` is a clock that could not be read, which is never zero: epoch zero
/// parses cleanly and ages everything to fifty-six years, so every reading
/// that depends on a clock drops out instead. Statements: S090.
/// Checked against `now_secs` (`src/main.rs:2752`) and `SystemProbes::now_secs`
/// (`src/system.rs:458`), which both answer `Option<u64>`.
pub trait Clock {
    fn now_secs(&self) -> Option<u64>;
}
