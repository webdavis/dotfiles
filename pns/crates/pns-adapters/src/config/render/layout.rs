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
    /// The tables nested INSIDE this one, written after its own keys because
    /// a TOML sub-heading ends the table above it.
    ///
    /// DATA, LIKE EVERY OTHER PART OF THIS LAYOUT: `[plugins.hermes.keys]` is
    /// a nested table whose vocabulary is the route names, and declaring it
    /// here is what keeps the walk in `render` one walk. A child inherits its
    /// parent's `present`, because a nested table under a commented-out
    /// heading has to be commented too, and its `opt_in` is therefore unread.
    pub children: &'static [Table],
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
pub(super) use delivery::EXAMPLE_CLASS;
use delivery::{DELIVERY, DELIVERY_CLASS};
mod core;
use core::*;
mod destinations;
use destinations::*;
mod lights;
use lights::*;
mod sensors;
use sensors::*;

/// Every table this layout declares, nested children included and in walk
/// order: a guard over the layout has to reach the same set of headings the
/// render writes, and `LAYOUT` alone stops at the outermost level.
#[cfg(test)]
pub(super) fn every_table() -> Vec<&'static Table> {
    fn push(into: &mut Vec<&'static Table>, tables: &'static [Table]) {
        for table in tables {
            into.push(table);
            push(into, table.children);
        }
    }
    let mut all = Vec::new();
    push(&mut all, LAYOUT);
    all
}

/// Every table this schema serves, in the order the file writes them.
pub(super) const LAYOUT: &[Table] = &[
    // FIRST, because it names the routes every table below is keyed by.
    ROUTES,
    PLUGINS_MOBILE,
    PLUGINS_HERMES,
    PLUGINS_DISCORD,
    PLUGINS_BANNER,
    PLUGINS_LIGHTS,
    PLUGINS_GITHUB,
    PLUGINS_PRESENCE,
    PLUGINS_HOME_PRESENCE,
    DAEMON,
    DELIVERY,
    DELIVERY_CLASS,
    RECAP,
    FOCUS,
    QUIET,
    REMIND,
    PRODUCER,
    STALE,
    PHONE,
    PATHS,
    STORAGE,
    FAILURES,
    LIGHTS,
    LIGHTS_DONE,
    LIGHTS_FAILED,
    LIGHTS_BLOCKED,
    LIGHTS_UNREAD,
    LIGHTS_GITHUB,
    LIGHTS_LOOP,
    LIGHTS_DIM,
];
