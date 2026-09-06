//! The daemon's job policy: what a job is, when it is due, and the bounds a
//! registration is held to.
//!
//! Only the id bound has arrived so far, because the nag's own name grammar is
//! derived from it and cannot reach back into the legacy package. The rest of
//! the job policy follows.

/// The most an id or a marker name may be. Long enough for a session id with
/// a prefix on it, short enough that a spool filename stays a filename.
pub const ID_MAX: usize = 64;
