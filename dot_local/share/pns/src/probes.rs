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

#[cfg(test)]
mod tests {
    use super::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};
    use crate::presence::{
        DEFAULT_ATTENTION_FLOOR_BYTES, DEFAULT_PHONE_MARKER_TTL_SECS, DEFAULT_PHYSICAL_FRESH_SECS,
        attention_band, marker_fresh, mosh_rate_active, moshi_viewing, phone_attention,
    };
    use crate::routing::viewed_pane_redundant;

    /// One stand-in for all four readings, so a test names only the readings
    /// its behavior is about.
    #[derive(Default)]
    struct FakeProbes {
        idle_secs: Option<u64>,
        marker_mtime_secs: Option<u64>,
        sample_csv: Option<String>,
        focused_pane: Option<String>,
    }

    impl IdleProbe for FakeProbes {
        fn idle_secs(&self) -> Option<u64> {
            self.idle_secs
        }
    }

    impl PhoneMarkerProbe for FakeProbes {
        fn marker_mtime_secs(&self) -> Option<u64> {
            self.marker_mtime_secs
        }
    }

    impl MoshRateProbe for FakeProbes {
        fn sample_csv(&self) -> Option<String> {
            self.sample_csv.clone()
        }
    }

    impl FocusedPaneProbe for FakeProbes {
        fn focused_pane(&self) -> Option<String> {
            self.focused_pane.clone()
        }
    }

    #[test]
    fn an_idle_reading_drives_the_band_through_the_probe_seam() {
        let probes = FakeProbes {
            idle_secs: Some(60),
            ..FakeProbes::default()
        };
        assert!(attention_band(
            probes.idle_secs(),
            Some(120),
            Some(DEFAULT_PHYSICAL_FRESH_SECS)
        ));
    }

    #[test]
    fn an_idle_probe_that_could_not_read_puts_the_reading_in_no_band() {
        let probes = FakeProbes::default();
        assert!(!attention_band(
            probes.idle_secs(),
            Some(120),
            Some(DEFAULT_PHYSICAL_FRESH_SECS)
        ));
    }

    #[test]
    fn a_marker_probe_that_could_not_read_fails_closed() {
        let probes = FakeProbes::default();
        assert!(!marker_fresh(
            probes.marker_mtime_secs(),
            Some(1_100),
            Some(DEFAULT_PHONE_MARKER_TTL_SECS)
        ));
    }

    #[test]
    fn a_rate_probe_that_could_not_sample_is_not_viewing() {
        let probes = FakeProbes::default();
        let rate_active = probes
            .sample_csv()
            .is_some_and(|csv| mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
        assert!(!moshi_viewing(None, rate_active));
    }

    #[test]
    fn a_sampled_rate_reaches_attention_through_the_probe_seam() {
        let probes = FakeProbes {
            sample_csv: Some(
                "01:00:00,mosh-server.111,,,1000,5000\n01:00:01,mosh-server.111,,,9000,5000\n"
                    .to_string(),
            ),
            ..FakeProbes::default()
        };
        let rate_active = probes
            .sample_csv()
            .is_some_and(|csv| mosh_rate_active(&csv, DEFAULT_ATTENTION_FLOOR_BYTES));
        assert!(phone_attention(
            None,
            false,
            moshi_viewing(None, rate_active)
        ));
    }

    #[test]
    fn a_focused_pane_probe_that_could_not_read_leaves_the_card_firing() {
        let probes = FakeProbes::default();
        assert!(!viewed_pane_redundant(
            "wW:p21",
            &probes.focused_pane().unwrap_or_default()
        ));
    }

    #[test]
    fn a_focused_pane_reading_that_matches_the_event_makes_the_card_redundant() {
        let probes = FakeProbes {
            focused_pane: Some("wW:p21".to_string()),
            ..FakeProbes::default()
        };
        assert!(viewed_pane_redundant(
            "wW:p21",
            &probes.focused_pane().unwrap_or_default()
        ));
    }
}
