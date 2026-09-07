//! The hue channel, native: flash the configured rooms green or red for a
//! long command's exit code.
//!
//! Hue is config-SELECTED but not event-dispatched: the pulse fires on an
//! exit code from the long-command notifier, not on a notification, so this
//! channel is the binary's `pulse` mode reading the same `[plugins.hue]`
//! table (bridge, key, rooms) rather than a leg of the event plan.
//!
//! ONE PUT PER ROOM, and the bridge does the rest. It speaks CLIP v2 directly,
//! and the `on_off_color` signal it takes flashes a grouped light for a
//! duration and then puts the lamp back itself, so nothing here snapshots a
//! light, sequences a ramp or writes a restore. Every absence is a silent
//! no-op, and a failed pulse must never fail the caller.
//!
//! THE RESTORE IS MEASURED, not assumed. This paragraph used to assert it with
//! no source behind it, and the CLIP v2 specification says nothing either way
//! about what happens when a signal ends. The drill of 2026-09-01 put a signal
//! on a real lamp and read its full state back before and after, with the lamp
//! ON and again with it OFF: both times the bridge restored it byte for byte.
//! That is what this channel is built on, and it is why there is no snapshot
//! here and no restore engine anywhere.

// THE LAMP RESOLUTION POLICY moved to `pns-domain`, one file per question it
// answers. What stays here parses: the `[plugins.hue]` settings, the quiet
// window off a config string, and the bridge's own JSON listing.
use pns_domain::lamps::{Fixture, Muting, Routing, Showing, resolve};

mod inventory;
mod settings;
pub use inventory::{grouped_light_ids_for_rooms, inventory};
pub use settings::{DEFAULT_ROOMS, HueSettings, hue_settings, quiet_window};
mod bodies;
mod bridge;
mod render;

pub use bodies::{breath_arm_body, clear_body, fade_body, pulse_body};
pub use bridge::{
    BRIDGE_DEADLINE, Bridge, HuePulse, Reading, TYPED_COMMAND_DEADLINE, UreqBridge, clear_held,
    fixture_path, resolve_on_bridge, signal_fixtures,
};
pub use render::{held_render, pulse_render};

#[cfg(test)]
mod tests;
