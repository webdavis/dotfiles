//! The one config serializer. `render` walks `LAYOUT`, a static table of every
//! table and key the schema serves, in file order, and turns a values table
//! into the text `pns setup` writes and (once a values file exists) the
//! shipped template regenerates from.
//!
//! THE LAYOUT IS DATA, NOT CODE: a table lists its heading, whether it is
//! CORE (always written live) or OPT-IN (written commented when the caller
//! never mentions it), and its keys, each carrying its own comment and either
//! a real `Default` (written live when the caller leaves it out) or an
//! `Example` (written commented until the caller's own value fills it in,
//! even inside a table that is otherwise live). NO `kind` FIELD: the value's
//! own TOML type decides how it is written, so the same walk renders a bool,
//! an integer, a string, a string array or a keepassxc secret without the
//! layout naming which one a key must be.
//!
//! `render` is the other half: it CONSUMES `values` as it writes, removing
//! every key and table it recognises, and refuses BY NAME anything left over
//! once the walk is done. A values file cannot smuggle an unknown key past
//! this roster any more than a loaded config can past `config`'s.

/// One table, in file order.
pub(super) struct Table {
    /// The heading it writes, dotted (`"plugins.mobile"`, `"lights.done"`),
    /// or a bare top-level name (`"daemon"`).
    pub name: &'static str,
    /// The comment above the heading. Carries its own `# ` prefixes and
    /// trailing newline, the way the wizard's old section constants did.
    pub prose: &'static str,
    /// CORE tables are always written live; OPT-IN tables are written
    /// commented, heading included, when the caller's values never mention
    /// them at all.
    pub opt_in: bool,
    pub keys: &'static [Key],
}

/// One key inside a table.
pub(super) struct Key {
    pub name: &'static str,
    /// The comment above the key line, or `""` for a key nothing says more
    /// about than its table already has.
    pub prose: &'static str,
    pub sample: Sample,
}

/// What a key falls back to when the caller's values do not mention it.
pub(super) enum Sample {
    /// Written LIVE, at this literal, in a table that is itself live.
    Default(&'static str),
    /// Written COMMENTED at this literal, even inside a live table, because
    /// there is no real default: the working setting is unset.
    Example(&'static str),
}

use super::prose::*;
mod delivery;
use delivery::DELIVERY;
mod core;
use core::*;
mod destinations;
use destinations::*;
mod lights;
use lights::*;
mod sensors;
use sensors::*;

/// Every table this schema serves, in the order the file writes them.
pub(super) const LAYOUT: &[Table] = &[
    PLUGINS_MOBILE,
    PLUGINS_HERMES,
    PLUGINS_MACOS_BANNER,
    PLUGINS_HUE,
    PLUGINS_PRESENCE,
    PLUGINS_ROUTER,
    DAEMON,
    DELIVERY,
    RECAP,
    FOCUS,
    NAG,
    FAILURES,
    LIGHTS,
    LIGHTS_DONE,
    LIGHTS_FAILED,
    LIGHTS_BLOCKED,
    LIGHTS_UNREAD,
    LIGHTS_LOOP,
    LIGHTS_DIM,
];
