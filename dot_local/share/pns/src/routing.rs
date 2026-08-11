//! WHICH destinations an event reaches, and whether the phone is one of them.
//!
//! The plan names NO channel. It is computed over the routing DECLARATIONS of
//! whatever plugins the registry selected, which is what closed the old
//! enum's open/closed violation: adding a destination is a registration, not
//! an edit here.

use crate::registry::Registration;

/// Whether the engine waits for a channel to finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Async,
    Sync,
}

impl Mode {
    /// The mode as the channel contract spells it in the event.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Async => "async",
            Mode::Sync => "sync",
        }
    }
}

/// One leg of a plan: the plugin's name, and the mode it is handed the event
/// in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub name: &'static str,
    pub mode: Mode,
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
    enabled: &[Registration],
    local_only: bool,
    remote_only: bool,
    want_phone: bool,
) -> Vec<Leg> {
    let _ = (enabled, local_only, remote_only, want_phone);
    todo!("R2c: compose the plan from routing declarations")
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
    use super::{Leg, Mode, channel_plan, viewed_pane_redundant, wants_phone};
    use crate::registry::{Registration, Routing};

    fn leg(name: &'static str, mode: Mode) -> Leg {
        Leg { name, mode }
    }

    /// The three real declarations, in the delivery order the bash engine
    /// uses: phone, log, banner. The plan tests run against these so every
    /// R1 behavior is preserved verbatim under the declaration API.
    fn three_enabled() -> Vec<Registration> {
        vec![
            Registration {
                name: "moshi",
                routing: Routing {
                    local: false,
                    presence_gated: true,
                    durable: false,
                },
            },
            Registration {
                name: "hermes",
                routing: Routing {
                    local: false,
                    presence_gated: false,
                    durable: true,
                },
            },
            Registration {
                name: "macos-banner",
                routing: Routing {
                    local: true,
                    presence_gated: false,
                    durable: false,
                },
            },
        ]
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
            channel_plan(&three_enabled(), false, false, true),
            vec![
                leg("moshi", Mode::Async),
                leg("hermes", Mode::Async),
                leg("macos-banner", Mode::Async),
            ]
        );
    }

    #[test]
    fn a_suppressed_phone_drops_only_the_presence_gated_leg() {
        assert_eq!(
            channel_plan(&three_enabled(), false, false, false),
            vec![leg("hermes", Mode::Async), leg("macos-banner", Mode::Async)]
        );
    }

    #[test]
    fn local_only_plans_the_local_surfaces_alone_whatever_the_phone_verdict_was() {
        // Both phone verdicts, because the flag is what decides this plan. Ask
        // only with the phone wanted and a narrowing that quietly reads the
        // phone verdict as well still answers correctly here.
        assert_eq!(
            channel_plan(&three_enabled(), true, false, true),
            vec![leg("macos-banner", Mode::Async)]
        );
        assert_eq!(
            channel_plan(&three_enabled(), true, false, false),
            vec![leg("macos-banner", Mode::Async)]
        );
    }

    #[test]
    fn remote_only_plans_the_durable_legs_alone_and_sync_which_keeps_a_lost_entry_visible() {
        // The suppressed-phone form is the one that pins SYNC to the flag
        // alone. Without it a narrowing that also consulted the phone verdict
        // would drop this plan back to the ordinary async pair, and a log
        // entry nobody waited for is the invisible loss sync exists to stop.
        assert_eq!(
            channel_plan(&three_enabled(), false, true, true),
            vec![leg("hermes", Mode::Sync)]
        );
        assert_eq!(
            channel_plan(&three_enabled(), false, true, false),
            vec![leg("hermes", Mode::Sync)]
        );
    }

    #[test]
    fn both_narrowing_flags_plan_nothing_at_all() {
        assert_eq!(channel_plan(&three_enabled(), true, true, true), vec![]);
        assert_eq!(channel_plan(&three_enabled(), true, true, false), vec![]);
    }

    #[test]
    fn no_enabled_plugins_plan_nothing_under_every_flag() {
        // An unconfigured machine has an empty plan, not a crash and not a
        // built-in fallback: the caller reports the empty verdict.
        for (local, remote, phone) in [
            (false, false, true),
            (false, false, false),
            (true, false, true),
            (false, true, true),
        ] {
            assert_eq!(channel_plan(&[], local, remote, phone), vec![]);
        }
    }

    #[test]
    fn the_presence_gate_means_one_thing_under_every_flag() {
        // A hypothetical presence-gated LOCAL plugin (a wearable's buzz) is
        // dropped by a no-phone verdict even on the local-only path: the gate
        // composes with the flags rather than living inside one branch.
        let gated_local = vec![Registration {
            name: "buzz",
            routing: Routing {
                local: true,
                presence_gated: true,
                durable: false,
            },
        }];
        assert_eq!(
            channel_plan(&gated_local, true, false, true),
            vec![leg("buzz", Mode::Async)]
        );
        assert_eq!(channel_plan(&gated_local, true, false, false), vec![]);
    }

    #[test]
    fn a_mode_names_what_the_channel_contract_spells_in_the_event() {
        assert_eq!(Mode::Async.as_str(), "async");
        assert_eq!(Mode::Sync.as_str(), "sync");
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
