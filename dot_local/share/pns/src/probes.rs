//! The IO boundary: every reading the core needs, and nothing else.
//!
//! THE TRAITS MOVED to `pns-application`, where the use cases that consume
//! them declare their own ports. What is left here is the path this package's
//! callers already name.

pub use pns_application::ports::environment::{
    IdleProbe, PhoneInputProbe, PhoneMarkerProbe, ProbeStart, ScreenLockProbe, SessionViewProbe,
    Wants,
};
