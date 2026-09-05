//! WHICH destinations an event reaches, and whether the phone is one of them.
//!
//! The plan names NO channel. It is computed over the routing DECLARATIONS of
//! whatever CHANNELS the registry selected, which is what closed the old
//! enum's open/closed violation: adding a destination is a registration, not
//! an edit here. A selected plugin of any other kind holds no declaration, so
//! it is not something the plan can reach.

use crate::registry::{PluginKind, Selection};

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

/// One leg of a plan: the plugin's name, the mode it is handed the event in,
/// and whether it is there because the plan DECORATED something.
///
/// `decorative` IS CARRIED RATHER THAN RECOMPUTED. Only this module knows why
/// a leg survived the plan, and the answer is in the declarations it filtered
/// on: a presence-gated plugin is here because the plan wanted a card, a local
/// one because it wanted a banner, and anything else is the durable log, which
/// every event reaches whatever the operator can see. A caller that needs to
/// know whether an operator will SEE this dispatch (the missed-notification
/// replay is the one that does) would otherwise have to name plugins or read
/// the declarations a second time, and `channel_plan`'s own comment says where
/// that ends: two copies of one policy, drifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub name: &'static str,
    pub mode: ReportMode,
    /// Whether the operator is shown something by this leg firing.
    pub decorative: bool,
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
        // ROUTING IS WHAT A LEG IS PLANNED FROM, and only a channel has any.
        // A sensor is an input: it holds no declaration to read, so no flag,
        // no fallback and no kind added later can turn one into a leg. The
        // match stays exhaustive so a third kind has to state its answer here
        // rather than inherit delivery from a catch-all.
        .filter_map(|entry| match entry.kind {
            PluginKind::Channel(routing) => Some((entry.name, routing)),
            PluginKind::Sensor => None,
        })
        // A plugin the binary serves in its own mode is not a destination an
        // event can reach, whatever the config selected it for.
        .filter(|(_, routing)| routing.event_dispatched)
        .filter(|(_, routing)| match (local_only, remote_only) {
            (true, _) => routing.local,
            (_, true) => routing.durable,
            _ => true,
        })
        // THE PLAN decides which surfaces an event reaches; the declarations
        // decide which plugin is which surface. A presence-gated plugin is the
        // phone, a local one is this machine's own screen, and anything else
        // is the durable log, which every event reaches.
        //
        // ONE READING, TWO ANSWERS. Whether the leg survives and whether it is
        // a DECORATION are the same two declarations asked in the same place,
        // so the second answer rides out on the leg rather than being derived
        // again somewhere that cannot see the declarations.
        .filter_map(|(name, routing)| {
            let decorative = routing.presence_gated || routing.local;
            let wanted = if routing.presence_gated {
                delivery.phone_card
            } else if routing.local {
                delivery.banner
            } else {
                true
            };
            wanted.then_some(Leg {
                name,
                mode,
                decorative,
            })
        })
        .collect()
}
