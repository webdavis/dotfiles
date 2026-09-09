use layout::{LAYOUT, Sample, Table};
use prose::*;
mod layout;
mod prose;
mod values;
use values::{quoted, render_value};
mod write;
use write::{render_block, render_core, render_opt_in, take_note, take_table, write_note};
mod lights;
use lights::render_lights;
mod secret;
use secret::{SECRET_FIELDS, secret_action};

/// The whole config text, built off a values table shaped like the config
/// itself: `{ plugins = { mobile = { ... }, ... }, recap = { ... }, ... }`.
///
/// EVERY KEY AND TABLE IN `values` IS CONSUMED as it is written, so anything
/// left over once the walk is done is a name this schema does not serve, and
/// the whole render is refused rather than silently dropping it.
pub fn render(values: &toml::Table) -> Result<String, String> {
    let mut remaining = values.clone();
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');

    let mut plugins = take_table(&mut remaining, "plugins")?;
    // LAYOUT IS WALKED ONCE, IN ITS OWN ORDER, and `opt_in` is what decides
    // `render_core` versus `render_opt_in` for every table: nothing here
    // hand-picks a table by name, so a table added, reordered or flipped in
    // LAYOUT changes what this walk writes without a matching edit here.
    // `lights` IS THE ONE HARDCODED BRANCH, because its seven headings share
    // one presence flag rather than each carrying its own; every
    // `lights.<x>` entry is written by that one call and skipped here.
    for table in LAYOUT {
        if table.name == "lights" {
            render_lights(&mut out, &mut remaining)?;
        } else if table.name.starts_with("lights.") {
            continue;
        } else if table.name.starts_with("plugins.") {
            if table.opt_in {
                render_opt_in(&mut out, table, &mut plugins)?;
            } else {
                render_core(&mut out, table, &mut plugins)?;
            }
        } else if table.opt_in {
            render_opt_in(&mut out, table, &mut remaining)?;
        } else {
            render_core(&mut out, table, &mut remaining)?;
        }
    }
    if let Some(name) = plugins.keys().next() {
        return Err(format!("unknown plugin `{name}`"));
    }

    out.push_str(TRAILER);

    if let Some(name) = remaining.keys().next() {
        return Err(format!("unknown top-level key `{name}`"));
    }
    Ok(out)
}

/// `Answers::values()` composes a table with no `daemon`, `recap` or
/// `lights` key at all, which is exactly what a CORE table's own default
/// path is for: absent means "unmodified," never "off."
fn find_table(name: &str) -> &'static Table {
    LAYOUT
        .iter()
        .find(|table| table.name == name)
        .unwrap_or_else(|| panic!("`{name}` is declared nowhere in LAYOUT"))
}

mod stub;
pub use stub::{identity_placeholder, strip_chezmoi_actions};

#[cfg(test)]
mod tests;
