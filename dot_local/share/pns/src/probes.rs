//! The IO boundary: every reading the core needs, and nothing else.
//!
//! Each trait is deliberately NARROW, one reading per trait, so a test
//! substitutes exactly the reading it is about and the core never grows a path
//! that touches the outside world. The concrete implementations (the idle
//! counter, the marker's timestamp, the sampler, the multiplexer query) belong
//! to the binary's composition root.
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

/// A two-sample CSV reading of per-session byte counters.
pub trait MoshRateProbe {
    fn sample_csv(&self) -> Option<String>;
}

/// The pane the multiplexer currently has focused. Focus is mirrored across
/// every attached client, so this is also what a phone viewing the session is
/// looking at.
pub trait FocusedPaneProbe {
    fn focused_pane(&self) -> Option<String>;
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

impl<T: MoshRateProbe> MoshRateProbe for &T {
    fn sample_csv(&self) -> Option<String> {
        (*self).sample_csv()
    }
}

impl<T: FocusedPaneProbe> FocusedPaneProbe for &T {
    fn focused_pane(&self) -> Option<String> {
        (*self).focused_pane()
    }
}
