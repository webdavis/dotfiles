//! WHICH destinations an event reaches, and whether the phone is one of them.

/// A destination the plan can name.
///
/// KNOWN LIMIT, and the one the rewrite has to close: the plan NAMES its
/// channels, so adding a destination means editing core policy rather than only
/// dropping an executable in the channels directory. That is an open/closed
/// violation and the same coupling that would make an extracted crate useless
/// to anyone whose stack is not this one. Closing it needs channels to declare
/// their own routing, which is a registration mechanism; the limit is named
/// here rather than half-solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// The phone card, and the only leg presence can suppress.
    Moshi,
    /// The durable log.
    Hermes,
    /// The desktop banner.
    MacosBanner,
}

impl Channel {
    /// The channel's name on disk: the engine looks for an executable of this
    /// name in the channels directory.
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Moshi => "moshi",
            Channel::Hermes => "hermes",
            Channel::MacosBanner => "macos-banner",
        }
    }
}

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

/// One channel of a plan, plus the mode it is handed the event in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub channel: Channel,
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

/// The channels that should fire, in delivery order.
///
/// An EMPTY plan means nothing fires, which is a legitimate verdict the caller
/// has to report rather than pass over in silence.
///
/// Remote-only is the LOG path: the durable channel alone, and SYNCHRONOUSLY,
/// because an undelivered log entry is invisible in a way an undelivered alert
/// is not. Local-only is its mirror and keeps the banner. Giving both
/// suppresses everything, which is why the caller must say so.
pub fn channel_plan(local_only: bool, remote_only: bool, want_phone: bool) -> Vec<Leg> {
    if local_only && remote_only {
        return Vec::new();
    }
    if remote_only {
        return vec![Leg {
            channel: Channel::Hermes,
            mode: Mode::Sync,
        }];
    }
    let mut plan = Vec::new();
    if want_phone && !local_only {
        plan.push(Leg {
            channel: Channel::Moshi,
            mode: Mode::Async,
        });
    }
    if !local_only {
        plan.push(Leg {
            channel: Channel::Hermes,
            mode: Mode::Async,
        });
    }
    plan.push(Leg {
        channel: Channel::MacosBanner,
        mode: Mode::Async,
    });
    plan
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
    use super::{Channel, Leg, Mode, channel_plan, viewed_pane_redundant, wants_phone};

    fn leg(channel: Channel, mode: Mode) -> Leg {
        Leg { channel, mode }
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
            channel_plan(false, false, true),
            vec![
                leg(Channel::Moshi, Mode::Async),
                leg(Channel::Hermes, Mode::Async),
                leg(Channel::MacosBanner, Mode::Async),
            ]
        );
    }

    #[test]
    fn a_suppressed_phone_leaves_the_other_two_untouched() {
        assert_eq!(
            channel_plan(false, false, false),
            vec![
                leg(Channel::Hermes, Mode::Async),
                leg(Channel::MacosBanner, Mode::Async)
            ]
        );
    }

    #[test]
    fn local_only_plans_the_banner_alone_whatever_the_phone_verdict_was() {
        // Both phone verdicts, because the flag is what decides this plan. Ask
        // only with the phone wanted and a narrowing that quietly reads the
        // phone verdict as well still answers correctly here.
        assert_eq!(
            channel_plan(true, false, true),
            vec![leg(Channel::MacosBanner, Mode::Async)]
        );
        assert_eq!(
            channel_plan(true, false, false),
            vec![leg(Channel::MacosBanner, Mode::Async)]
        );
    }

    #[test]
    fn remote_only_plans_the_log_alone_and_sync_which_keeps_a_lost_entry_visible() {
        // The suppressed-phone form is the one that pins SYNC to the flag
        // alone. Without it a narrowing that also consulted the phone verdict
        // would drop this plan back to the ordinary async pair, and a log
        // entry nobody waited for is the invisible loss sync exists to stop.
        assert_eq!(
            channel_plan(false, true, true),
            vec![leg(Channel::Hermes, Mode::Sync)]
        );
        assert_eq!(
            channel_plan(false, true, false),
            vec![leg(Channel::Hermes, Mode::Sync)]
        );
    }

    #[test]
    fn both_narrowing_flags_plan_nothing_at_all() {
        assert_eq!(channel_plan(true, true, true), vec![]);
        assert_eq!(channel_plan(true, true, false), vec![]);
    }

    #[test]
    fn a_channel_names_the_executable_the_engine_looks_for() {
        assert_eq!(Channel::Moshi.as_str(), "moshi");
        assert_eq!(Channel::Hermes.as_str(), "hermes");
        assert_eq!(Channel::MacosBanner.as_str(), "macos-banner");
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
