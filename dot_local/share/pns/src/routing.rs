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

#[cfg(test)]
mod tests {
    use super::{Leg, ReportMode, channel_plan};
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

    // --- channel_plan ------------------------------------------------------

    #[test]
    fn the_alert_path_plans_phone_then_banner_then_log() {
        // The two presence-sensitive surfaces come first and the durable log
        // last: the plan is computed from one reading of where the operator
        // is, and hermes posts over the network under its own deadline.
        assert_eq!(
            channel_plan(&three_enabled(), false, false, reaching(true, true)),
            vec![
                leg("moshi", ReportMode::Silent),
                leg("macos-banner", ReportMode::Silent),
                leg("hermes", ReportMode::Silent),
            ]
        );
    }

    #[test]
    fn a_suppressed_phone_drops_only_the_presence_gated_leg() {
        assert_eq!(
            channel_plan(&three_enabled(), false, false, reaching(true, false)),
            vec![
                leg("macos-banner", ReportMode::Silent),
                leg("hermes", ReportMode::Silent)
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
}
