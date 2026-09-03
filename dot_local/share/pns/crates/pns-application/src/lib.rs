//! The PNS use cases, and the ports they own.
//!
//! This crate is responsible for orchestrating one operator-meaningful
//! operation at a time (submitting a notification, requesting an approval,
//! replaying missed notifications, building a return recap, running a nag,
//! reading the home probe, reconciling the lights, taking a loop lease,
//! running a daemon tick, scheduling or cancelling a job, running the doctor,
//! running setup) by combining pns-domain policy with capabilities it declares
//! and does not implement.
//!
//! Those declarations are the point. A use case owns the trait it consumes, so
//! the dependency runs inward: adapters implement these ports, and this crate
//! never names an adapter. It constructs no HTTP client, spawns no process,
//! opens no file and reads no environment variable.
//!
//! Nothing has moved in yet.
