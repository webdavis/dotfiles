//! WHICH destinations an event reaches, and whether the phone is one of them.
//!
//! The plan names NO channel. It is computed over the routing DECLARATIONS of
//! whatever plugins the registry selected, which is what closed the old
//! enum's open/closed violation: adding a destination is a registration, not
//! an edit here.

use crate::registry::Selection;

/// Whether a leg's outcome is reported to the operator.
///
/// It used to be Async and Sync, which claimed a waiting semantic nothing has:
/// shell dispatch always waits for the channel to exit, and the native HTTP
/// calls block too. What actually differs is whether the leg says how it went,
/// and, for hermes alone, which deadline it posts under.
///
/// THE WIRE WORDS DO NOT CHANGE. `as_str` still emits `async` and `sync`,
/// because that is what the channel contract has always carried and what the
/// executable channels read; renaming it there would be a behavior change to
/// every channel this binary does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    /// Deliver and say nothing.
    Silent,
    /// Deliver and report what happened.
    ReportOutcome,
}

impl ReportMode {
    /// The mode as the channel contract spells it in the event.
    pub fn as_str(self) -> &'static str {
        match self {
            ReportMode::Silent => "async",
            ReportMode::ReportOutcome => "sync",
        }
    }
}

/// One leg of a plan: the plugin's name, and the mode it is handed the event
/// in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub name: &'static str,
    pub mode: ReportMode,
}

/// True when the phone push should fire.
///
/// FAIL OPEN ON ANY UNCERTAINTY. An unreadable idle clock or threshold means
/// presence is UNKNOWN, and unknown must mean "treat as away" so a push is
/// never silently dropped; a reading coerced to zero would instead read as
/// "actively typing" and suppress it. Either narrowing flag suppresses the
/// phone outright, and the force override beats presence but not the flags.
pub fn wants_phone(
    idle_secs: Option<u64>,
    desk_idle_secs: Option<u64>,
    local_only: bool,
    remote_only: bool,
    force: bool,
) -> bool {
    if local_only || remote_only {
        return false;
    }
    if force {
        return true;
    }
    let (Some(idle_secs), Some(desk_idle_secs)) = (idle_secs, desk_idle_secs) else {
        return true;
    };
    idle_secs >= desk_idle_secs
}

/// The legs that should fire, in the registry's delivery order.
///
/// An EMPTY plan means nothing fires, which is a legitimate verdict the caller
/// has to report rather than pass over in silence.
///
/// The rules compose over declarations, never names. Remote-only is the LOG
/// path: the durable plugins alone, and SYNCHRONOUSLY, because an undelivered
/// log entry is invisible in a way an undelivered alert is not. Local-only is
/// its mirror and keeps the local surfaces. Giving both suppresses
/// everything, which is why the caller must say so. A presence-gated plugin
/// is dropped whenever the phone verdict is no, under every flag, so the gate
/// means one thing everywhere.
pub fn channel_plan(
    enabled: &Selection,
    local_only: bool,
    remote_only: bool,
    delivery: crate::surface::DeliveryPlan,
) -> Vec<Leg> {
    if local_only && remote_only {
        return Vec::new();
    }
    let mode = if remote_only {
        ReportMode::ReportOutcome
    } else {
        ReportMode::Silent
    };
    enabled
        .iter()
        // A plugin the binary serves in its own mode is not a destination an
        // event can reach, whatever the config selected it for.
        .filter(|entry| entry.routing.event_dispatched)
        .filter(|entry| match (local_only, remote_only) {
            (true, _) => entry.routing.local,
            (_, true) => entry.routing.durable,
            _ => true,
        })
        // THE PLAN decides which surfaces an event reaches; the declarations
        // decide which plugin is which surface. A presence-gated plugin is the
        // phone, a local one is this machine's own screen, and anything else
        // is the durable log, which every event reaches.
        .filter(|entry| {
            if entry.routing.presence_gated {
                delivery.phone_card
            } else if entry.routing.local {
                delivery.banner
            } else {
                true
            }
        })
        .map(|entry| Leg {
            name: entry.name,
            mode,
        })
        .collect()
}

/// True when a phone card would describe the very pane the operator is
/// watching: both ids are present and identical. herdr mirrors focus across
/// every attached client, so the focused pane IS what a phone viewing the
/// session shows. Either id missing is unknown, and unknown fails OPEN (the
/// card fires): a duplicate card costs a glance, a dropped one costs the event.
///
/// This is only half the verdict. The caller must ALSO hold proof the phone is
/// actively viewing; a phone in hand is not a pane on screen.
pub fn viewed_pane_redundant(event_pane: &str, focused_pane: &str) -> bool {
    !event_pane.is_empty() && !focused_pane.is_empty() && event_pane == focused_pane
}

#[cfg(test)]
mod tests {
    use super::{Leg, ReportMode, channel_plan, viewed_pane_redundant, wants_phone};
    use crate::config::parse_config;
    use crate::registry::{Registry, Routing, Selection};

    fn leg(name: &'static str, mode: ReportMode) -> Leg {
        Leg { name, mode }
    }

    /// A plan that reaches every surface, so a narrowing test varies one
    /// thing: the flags. `card` is the phone, `banner` this machine's screen.
    fn reaching(banner: bool, card: bool) -> crate::surface::DeliveryPlan {
        crate::surface::DeliveryPlan {
            banner,
            phone_card: card,
            pulse: false,
        }
    }

    /// Every selection comes out of a real registry and a real config, so
    /// each plan test exercises register, enabled and channel_plan END TO
    /// END: a registry that mislaid a routing declaration fails these, not
    /// only its own unit tests.
    fn select(registry: &Registry, config_text: &str) -> Selection {
        registry
            .enabled(&parse_config(config_text).unwrap())
            .unwrap()
    }

    const ALL_THREE_ON: &str = "[plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n";

    fn three_enabled() -> Selection {
        select(&crate::registry::test_roster(), ALL_THREE_ON)
    }

    // --- wants_phone -------------------------------------------------------

    #[test]
    fn away_from_the_desk_wants_the_phone() {
        assert!(wants_phone(Some(900), Some(600), false, false, false));
    }

    #[test]
    fn at_the_desk_does_not_want_the_phone() {
        assert!(!wants_phone(Some(60), Some(600), false, false, false));
    }

    #[test]
    fn idle_exactly_at_the_threshold_is_already_away() {
        assert!(wants_phone(Some(600), Some(600), false, false, false));
    }

    #[test]
    fn an_unreadable_idle_probe_fails_open_because_unknown_presence_must_not_drop_a_push() {
        assert!(wants_phone(None, Some(600), false, false, false));
    }

    #[test]
    fn an_unreadable_threshold_fails_open_too() {
        assert!(wants_phone(Some(60), None, false, false, false));
    }

    #[test]
    fn the_force_override_beats_presence() {
        assert!(wants_phone(Some(0), Some(600), false, false, true));
    }

    #[test]
    fn either_narrowing_flag_suppresses_the_phone_even_under_the_force_override() {
        assert!(!wants_phone(Some(900), Some(600), true, false, true));
        assert!(!wants_phone(Some(900), Some(600), false, true, true));
    }

    #[test]
    fn a_narrowing_flag_suppresses_the_phone_even_when_presence_is_unknown() {
        assert!(!wants_phone(None, None, true, false, false));
    }

    // --- channel_plan ------------------------------------------------------

    #[test]
    fn the_alert_path_plans_phone_then_log_then_banner() {
        assert_eq!(
            channel_plan(&three_enabled(), false, false, reaching(true, true)),
            vec![
                leg("moshi", ReportMode::Silent),
                leg("hermes", ReportMode::Silent),
                leg("macos-banner", ReportMode::Silent),
            ]
        );
    }

    #[test]
    fn a_suppressed_phone_drops_only_the_presence_gated_leg() {
        assert_eq!(
            channel_plan(&three_enabled(), false, false, reaching(true, false)),
            vec![
                leg("hermes", ReportMode::Silent),
                leg("macos-banner", ReportMode::Silent)
            ]
        );
    }

    #[test]
    fn local_only_plans_the_local_surfaces_alone_whatever_the_phone_verdict_was() {
        // Both phone verdicts, because the flag is what decides this plan. Ask
        // only with the phone wanted and a narrowing that quietly reads the
        // phone verdict as well still answers correctly here.
        assert_eq!(
            channel_plan(&three_enabled(), true, false, reaching(true, true)),
            vec![leg("macos-banner", ReportMode::Silent)]
        );
        assert_eq!(
            channel_plan(&three_enabled(), true, false, reaching(true, false)),
            vec![leg("macos-banner", ReportMode::Silent)]
        );
    }

    #[test]
    fn remote_only_plans_the_durable_legs_alone_and_sync_which_keeps_a_lost_entry_visible() {
        // The suppressed-phone form is the one that pins SYNC to the flag
        // alone. Without it a narrowing that also consulted the phone verdict
        // would drop this plan back to the ordinary async pair, and a log
        // entry nobody waited for is the invisible loss sync exists to stop.
        assert_eq!(
            channel_plan(&three_enabled(), false, true, reaching(true, true)),
            vec![leg("hermes", ReportMode::ReportOutcome)]
        );
        assert_eq!(
            channel_plan(&three_enabled(), false, true, reaching(true, false)),
            vec![leg("hermes", ReportMode::ReportOutcome)]
        );
    }

    #[test]
    fn both_narrowing_flags_plan_nothing_at_all() {
        assert_eq!(
            channel_plan(&three_enabled(), true, true, reaching(true, true)),
            vec![]
        );
        assert_eq!(
            channel_plan(&three_enabled(), true, true, reaching(true, false)),
            vec![]
        );
    }

    #[test]
    fn no_enabled_plugins_plan_nothing_under_every_flag() {
        // An unconfigured machine has an empty plan, not a crash and not a
        // built-in fallback: the caller reports the empty verdict.
        let none = select(&crate::registry::test_roster(), "");
        for (local, remote, phone) in [
            (false, false, true),
            (false, false, false),
            (true, false, true),
            (false, true, true),
        ] {
            assert_eq!(
                channel_plan(&none, local, remote, reaching(true, phone)),
                vec![]
            );
        }
    }

    #[test]
    fn a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected() {
        // hue is registered and selectable, and the pulse mode runs it, but no
        // notification may route to it. Without this the roster's fourth entry
        // would start appearing as a channel on every event.
        let enabled = select(
            &crate::registry::test_roster(),
            "[plugins.hue]\nenabled = true\n[plugins.hermes]\nenabled = true\n",
        );
        assert_eq!(
            channel_plan(&enabled, false, false, reaching(true, true)),
            vec![leg("hermes", ReportMode::Silent)]
        );
        assert_eq!(
            channel_plan(&enabled, true, false, reaching(true, true)),
            Vec::new(),
            "not even the local-only path, which hue would otherwise match"
        );
    }

    #[test]
    fn the_presence_gate_means_one_thing_under_every_flag() {
        // Two hypotheticals pin the gate as a COMPOSED filter rather than a
        // branch: a presence-gated LOCAL plugin (a wearable's buzz) on the
        // local-only path, and a presence-gated DURABLE plugin (a phone
        // pager log) on the remote-only path. An implementation that skips
        // the gate inside either flag's branch keeps one of them wrongly.
        let mut registry = Registry::new();
        registry
            .register(
                "buzz",
                Routing {
                    local: true,
                    presence_gated: true,
                    durable: false,
                    event_dispatched: true,
                },
            )
            .unwrap();
        registry
            .register(
                "pager",
                Routing {
                    local: false,
                    presence_gated: true,
                    durable: true,
                    event_dispatched: true,
                },
            )
            .unwrap();
        let both = "[plugins.buzz]\nenabled = true\n[plugins.pager]\nenabled = true\n";
        let enabled = select(&registry, both);

        assert_eq!(
            channel_plan(&enabled, true, false, reaching(true, true)),
            vec![leg("buzz", ReportMode::Silent)]
        );
        assert_eq!(
            channel_plan(&enabled, true, false, reaching(true, false)),
            vec![]
        );
        assert_eq!(
            channel_plan(&enabled, false, true, reaching(true, true)),
            vec![leg("pager", ReportMode::ReportOutcome)]
        );
        assert_eq!(
            channel_plan(&enabled, false, true, reaching(true, false)),
            vec![]
        );
    }

    #[test]
    fn a_mode_names_what_the_channel_contract_spells_in_the_event() {
        assert_eq!(ReportMode::Silent.as_str(), "async");
        assert_eq!(ReportMode::ReportOutcome.as_str(), "sync");
    }

    // --- viewed_pane_redundant ---------------------------------------------

    #[test]
    fn a_card_about_the_watched_pane_is_redundant_when_both_ids_agree() {
        assert!(viewed_pane_redundant("wW:p21", "wW:p21"));
    }

    #[test]
    fn a_different_focused_pane_is_not_redundant_so_that_card_must_fire() {
        assert!(!viewed_pane_redundant("wW:p21", "wW:p7"));
    }

    #[test]
    fn a_pane_id_that_merely_prefixes_the_focused_one_is_still_a_different_pane() {
        // Pane 2 and pane 21 share a prefix, so a comparison that only checks
        // the head suppresses the card about one while the operator watches
        // the other. That is the dropped event, and it costs more than the
        // duplicate card the fail-open direction accepts.
        assert!(!viewed_pane_redundant("wW:p21", "wW:p2"));
        assert!(!viewed_pane_redundant("wW:p2", "wW:p21"));
    }

    #[test]
    fn a_focused_pane_that_differs_only_in_case_is_still_a_different_pane() {
        // herdr's ids carry both cases (wW), so folding case merges two real
        // panes into one and suppresses a card about a pane nobody is reading.
        assert!(!viewed_pane_redundant("wW:p21", "ww:p21"));
    }

    #[test]
    fn an_event_without_a_pane_can_never_be_redundant() {
        assert!(!viewed_pane_redundant("", "wW:p21"));
    }

    #[test]
    fn unknown_focus_fails_open_so_no_focused_pane_means_not_redundant() {
        assert!(!viewed_pane_redundant("wW:p21", ""));
    }

    #[test]
    fn two_unknown_panes_are_not_redundant_even_though_they_match() {
        assert!(!viewed_pane_redundant("", ""));
    }
}
