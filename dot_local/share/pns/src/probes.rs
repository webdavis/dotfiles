//! The IO boundary: every reading the core needs, and nothing else.
//!
//! Each trait is deliberately NARROW, one reading per trait, so a test
//! substitutes exactly the reading it is about and the core never grows a path
//! that touches the outside world. The concrete implementations (the idle
//! counter, the marker's timestamp, the phone's pty clock, the console lock,
//! the multiplexer query) belong to the binary's composition root.
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

/// Whether the console screen is locked.
///
/// A separate reading from the idle clock beside it even though one command
/// answers both questions, because they are different facts: the idle clock
/// says how long ago the keyboard was touched, and this says whether touching
/// it again would reach the session at all.
///
/// `Some(true)` is locked, `Some(false)` unlocked, `None` unreadable, and the
/// decision (`surface::surface`) treats only `Some(true)` as locked.
pub trait ScreenLockProbe {
    fn screen_locked(&self) -> Option<bool>;
}

/// The session's display state, as the multiplexer sees it. One reading for
/// the whole event: herdr is the server and every client shows the same panes,
/// so what is on screen is a session-level fact and not a per-client one.
pub trait SessionViewProbe {
    /// `None` when any part of the view could not be read, which the model
    /// turns into Unknown, which never suppresses.
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView>;
}

/// Which of the subprocess-backed probes a caller is about to want.
///
/// One field per probe that is worth starting ahead of the read that will
/// join it; the marker and the clock are not here because neither spawns a
/// subprocess, and the session view is not here because it has exactly one
/// production reader already, with nothing to overlap it against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wants {
    pub desk: bool,
    pub phone: bool,
}

/// Begin the subprocess-backed readings a caller is about to want, so a slow
/// one runs in the background instead of blocking every reading that does
/// not depend on it.
///
/// A NO-OP BY DEFAULT: only `SystemProbes` overrides it, so every probe set
/// wired against a fixture, which is every test double in this crate, answers
/// exactly as fast started as unstarted, with nothing to overlap.
///
/// STARTING IS NEVER READING. A caller that never calls this still gets an
/// answer: every read computes its own reading inline when nothing was
/// started for it, which is what keeps this trait's absence from a probe set
/// a correctness question rather than only a performance one.
pub trait ProbeStart {
    fn start(&self, _wants: Wants) {}
}

impl<T: ProbeStart> ProbeStart for &T {
    fn start(&self, wants: Wants) {
        (*self).start(wants)
    }
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

impl<T: ScreenLockProbe> ScreenLockProbe for &T {
    fn screen_locked(&self) -> Option<bool> {
        (*self).screen_locked()
    }
}

impl<T: SessionViewProbe> SessionViewProbe for &T {
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView> {
        (*self).session_view(origin_pane)
    }
}
