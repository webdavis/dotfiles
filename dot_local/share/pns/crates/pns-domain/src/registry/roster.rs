//! The compiled-in roster and the selection policy beside it, as DATA.
//!
//! Separated from the registry that reads it so the declarations an operator
//! selects among sit in one file, and so adding a destination touches nothing
//! but this list.

use super::{PluginKind, Registration, Routing};

/// The declarations the composition root registers, named once so a test can
/// run against the real thing. Each entry states its KIND, so a sensor rides
/// in the same list as the channels rather than in a second one the
/// composition root has to remember.
pub const ROSTER: [Registration; 6] = [
    Registration {
        // The home probe's router: an INPUT, so it holds no delivery order to
        // state and sits ahead of the channels, whose order is delivery order.
        // `pns home` reads it; no event can route to it, because a sensor
        // carries no routing for a plan to read.
        name: "router",
        kind: PluginKind::Sensor,
    },
    Registration {
        // Which ROOM the operator is in, read off the state file the daemon
        // publishes. A second INPUT, beside the router and ahead of the
        // channels for the same reason. It borrows `[plugins.hue]`'s bridge
        // and key rather than declaring its own, which is what `REQUIRES`
        // above holds it to.
        name: PRESENCE,
        kind: PluginKind::Sensor,
    },
    Registration {
        // The phone. NAMED FOR THE DESTINATION, not for the service behind it:
        // `[plugins.mobile] type` names which backend carries the card (moshi
        // today), so a second one is a value the operator writes rather than a
        // second plugin name and a second table to move settings into.
        name: "mobile",
        kind: PluginKind::Channel(Routing {
            local: false,
            presence_gated: true,
            durable: false,
            event_dispatched: true,
        }),
    },
    Registration {
        // AHEAD OF THE DURABLE LOG, because this one is presence-sensitive
        // and that one is not. The plan is computed from a reading of where
        // the operator is at dispatch, and hermes can post synchronously
        // against a deadline; delivering the banner after it would show the
        // operator a decision taken about a moment that had passed.
        name: "macos-banner",
        kind: PluginKind::Channel(Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: true,
        }),
    },
    Registration {
        name: "hermes",
        kind: PluginKind::Channel(Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        }),
    },
    Registration {
        // A local surface the binary drives in its own `pulse` mode. It
        // registers so the config can select it and so a typo in its name is
        // still refused, but no event ever routes to it.
        name: "hue",
        kind: PluginKind::Channel(Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: false,
        }),
    },
];

/// WHICH PLUGIN BORROWS WHICH. A sensor that reads another plugin's
/// credential rather than declaring its own is refused when that other plugin
/// is off, because the alternative is a table the operator switched on that
/// silently never reads anything.
///
/// DATA BESIDE `CORE`, for the same reason: this is selection policy, and the
/// roster states what a plugin IS.
pub(super) const REQUIRES: [(&str, &str); 1] = [(PRESENCE, "hue")];

/// The room-presence sensor's config name, spelled once. Three modules select
/// on it (the roster, the doctor's own check, and the settings reader), and a
/// literal in each is three spellings to drift.
pub const PRESENCE: &str = "presence";

/// THE CORE: what a machine with no usable config runs. Names rather than a
/// flag on the declaration, because this is a selection policy and the roster
/// states what a plugin IS; a name here that nothing registers simply selects
/// nothing, which is what `the_core_is_two_registered_plugins_and_the_config_
/// still_beats_it` is for. IN REGISTRATION ORDER, so the warning that lists it
/// reads in the order the legs run.
pub const CORE: [&str; 2] = ["mobile", "macos-banner"];
