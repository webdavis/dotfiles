//! The IO boundary: every reading the core needs, and nothing else.
//!
//! Each trait is deliberately NARROW, one reading per trait, so a test
//! substitutes exactly the reading it is about and the core never grows a path
//! that touches the outside world. The concrete implementations (the idle
//! counter, the marker's timestamp, the phone's pty clock, the multiplexer
//! query) belong to the binary's composition root.
//!
//! Every reading is optional, because every one of them can fail to be taken.
//! `None` is "could not read", never a value, and each decision states its own
//! fail direction for it.

/// Seconds since the last physical input on this machine.
pub trait IdleProbe {
    fn idle_secs(&self) -> Option<u64>;
}

/// The modification time, in whole seconds, of the deliberate
/// "I am on my phone" marker.
pub trait PhoneMarkerProbe {
    fn marker_mtime_secs(&self) -> Option<u64>;
}

/// When the phone last put INPUT into the session, as the access time of the
/// mosh client's pty.
///
/// A timestamp rather than an age, exactly like the marker beside it: the
/// clock is read once at the edge and every reading is aged against that one
/// value, so two signals cannot be compared across two different "now"s.
pub trait PhoneInputProbe {
    fn phone_input_atime_secs(&self) -> Option<u64>;
}

/// The session's display state, as the multiplexer sees it. One reading for
/// the whole event: herdr is the server and every client shows the same panes,
/// so what is on screen is a session-level fact and not a per-client one.
pub trait SessionViewProbe {
    /// `None` when any part of the view could not be read, which the model
    /// turns into Unknown, which never suppresses.
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView>;
}

// A SHARED reading is one reading. The composition root builds one probe set
// per event and hands the same one to the engine and to every channel that
// needs a reading, so two consumers can never take the same measurement twice
// and disagree. These make `&Probes` satisfy the traits its owner does, which
// is what lets them share without moving ownership.
impl<T: IdleProbe> IdleProbe for &T {
    fn idle_secs(&self) -> Option<u64> {
        (*self).idle_secs()
    }
}

impl<T: PhoneMarkerProbe> PhoneMarkerProbe for &T {
    fn marker_mtime_secs(&self) -> Option<u64> {
        (*self).marker_mtime_secs()
    }
}

impl<T: PhoneInputProbe> PhoneInputProbe for &T {
    fn phone_input_atime_secs(&self) -> Option<u64> {
        (*self).phone_input_atime_secs()
    }
}

impl<T: SessionViewProbe> SessionViewProbe for &T {
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView> {
        (*self).session_view(origin_pane)
    }
}
