use super::*;

/// Removes `key` from `container` as a table, or an empty one when it is
/// absent, refusing by name when it is present but not a table.
pub(super) fn take_table(container: &mut toml::Table, key: &str) -> Result<toml::Table, String> {
    match container.remove(key) {
        None => Ok(toml::Table::new()),
        Some(toml::Value::Table(table)) => Ok(table),
        Some(other) => Err(format!(
            "`{key}` has type `{}`, not a table",
            other.type_str()
        )),
    }
}

pub(super) fn last_segment(dotted: &str) -> &str {
    dotted.rsplit('.').next().unwrap_or(dotted)
}

/// A CORE table: always written live, whether or not `container` mentions it.
pub(super) fn render_core(
    out: &mut String,
    table: &Table,
    container: &mut toml::Table,
) -> Result<(), String> {
    let mut settings = take_table(container, last_segment(table.name))?;
    render_block(out, table, &mut settings, true)
}

/// An OPT-IN table: written commented, heading and all, when `container`
/// never mentions it at all.
pub(super) fn render_opt_in(
    out: &mut String,
    table: &Table,
    container: &mut toml::Table,
) -> Result<(), String> {
    match container.remove(last_segment(table.name)) {
        None => render_block(out, table, &mut toml::Table::new(), false),
        Some(toml::Value::Table(mut settings)) => render_block(out, table, &mut settings, true),
        Some(other) => Err(format!(
            "`{}` has type `{}`, not a table",
            table.name,
            other.type_str()
        )),
    }
}

/// Writes one table's prose, heading and keys. `present` decides whether the
/// heading and every `Default` key are live or commented; an `Example` key
/// stays commented either way unless `settings` itself carries a value for
/// it, in which case that value always wins and is always written live.
pub(super) fn render_block(
    out: &mut String,
    table: &Table,
    settings: &mut toml::Table,
    present: bool,
) -> Result<(), String> {
    out.push_str(table.prose);
    write_note(out, take_note(settings)?);
    if present {
        out.push_str(&format!("[{}]\n", table.name));
    } else {
        out.push_str(&format!("# [{}]\n", table.name));
    }
    for key in table.keys {
        out.push_str(key.prose);
        match settings.remove(key.name) {
            Some(value) => {
                let rendered = render_value(&value)
                    .map_err(|error| format!("`{}` key `{}`: {error}", table.name, key.name))?;
                out.push_str(&format!("{} = {rendered}\n", key.name));
            }
            None => {
                let (literal, force_commented) = match key.sample {
                    Sample::Default(literal) => (literal, false),
                    Sample::Example(literal) => (literal, true),
                };
                if present && !force_commented {
                    out.push_str(&format!("{} = {literal}\n", key.name));
                } else {
                    out.push_str(&format!("# {} = {literal}\n", key.name));
                }
            }
        }
    }
    out.push('\n');
    if let Some(name) = settings.keys().next() {
        return Err(format!("unknown `{}` key `{name}`", table.name));
    }
    Ok(())
}

/// Removes and returns `note` off `settings`, refusing by name when it is
/// there but not a string.
///
/// A RESERVED KEY, invisible to the roster: it never reaches the output as
/// `note = "..."`, only as the comment `write_note` turns it into, so a
/// parsed config never carries one.
pub(super) fn take_note(settings: &mut toml::Table) -> Result<Option<String>, String> {
    match settings.remove("note") {
        None => Ok(None),
        Some(toml::Value::String(note)) => {
            // A NOTE IS A RAW COMMENT, never a quoted string, so `quoted`'s
            // brace-splitting cannot stand between it and chezmoi's template
            // engine: refuse the opening outright rather than write it.
            if note.contains("{{") {
                return Err("`note` cannot open a chezmoi template action".to_string());
            }
            // CRLF IS AN ORDINARY LINE ENDING, normalized before the control
            // check below so a pasted Windows-style note is accepted rather
            // than refused for the CR half of its own newline.
            let note = note.replace("\r\n", "\n");
            // ANY OTHER CONTROL CHARACTER, a lone CR included, rides straight
            // into `write_note`'s `# `-prefixed comment line and makes
            // `parse_config` refuse text this render just claimed worked.
            if note.chars().any(|character| {
                (character.is_control() || character == '\u{7f}') && character != '\n'
            }) {
                return Err("`note` cannot hold a control character".to_string());
            }
            Ok(Some(note))
        }
        Some(other) => Err(format!(
            "`note` has type `{}`, not a string",
            other.type_str()
        )),
    }
}

/// Writes `note` as one or more `# `-prefixed comment lines, or nothing at
/// all when there is none.
///
/// EVERY LINE GETS ITS OWN `# `, which is what keeps a newline inside the
/// operator's own text from opening a heading or an uncommented key: nothing
/// this function writes can ever start a line without that prefix, however
/// many newlines the note carries.
pub(super) fn write_note(out: &mut String, note: Option<String>) {
    let Some(note) = note else { return };
    if note.is_empty() {
        out.push_str("#\n");
        return;
    }
    for line in note.split('\n') {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
}
