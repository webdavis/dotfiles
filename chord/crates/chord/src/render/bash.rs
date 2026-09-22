//! Bash: readline `bind` calls, in the style of the file this replaces.

use super::{RenderFault, Renderer};
use crate::keys;
use crate::table::{Action, Binding, Group, Table};

/// What the file says about itself when the table names no header of its
/// own: chord wrote it, and the table is where an edit belongs.
const NEUTRAL_HEADER: &str = "\
# GENERATED FILE: `chord render bash` over a chord binding table.
# Edit the table and render again; an edit here is lost on the next render.
";

/// The keymap whose bindings must first leave vi's command mode, which is
/// what the leading `i` in a command-mode macro does.
const COMMAND_MODE: &str = "vi-command";

pub struct Bash;

impl Renderer for Bash {
    fn render(&self, table: &Table) -> Result<String, RenderFault> {
        let mut out = super::header(table.render.bash.header.as_deref(), NEUTRAL_HEADER);
        let clear_line = table.render.bash.clear_line.as_deref();
        for group in &table.group {
            render_group(group, clear_line, &mut out)?;
        }
        Ok(out)
    }

    fn output<'a>(&self, table: &'a Table) -> Option<&'a str> {
        table.render.bash.output.as_deref()
    }
}

fn render_group(
    group: &Group,
    clear_line: Option<&str>,
    out: &mut String,
) -> Result<(), RenderFault> {
    out.push_str(&format!("\n# --- {} ---\n", group.name));
    push_comment(group.description.as_deref(), out);
    for binding in &group.binding {
        out.push('\n');
        push_comment(binding.description.as_deref(), out);
        for line in render_binding(binding, clear_line)? {
            out.push_str(&line);
            out.push('\n');
        }
    }
    Ok(())
}

fn push_comment(text: Option<&str>, out: &mut String) {
    let Some(text) = text else { return };
    for line in text.trim_end().lines() {
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            out.push_str(&format!("# {line}\n"));
        }
    }
}

/// Every refusal the rows raise names the row it came from, which is the
/// only way the reader can find it in a table of hundreds.
fn render_binding(binding: &Binding, clear_line: Option<&str>) -> Result<Vec<String>, RenderFault> {
    bind_lines(binding, clear_line).map_err(|reason| fault(binding, &reason.0))
}

/// One `builtin bind` line per mode, or a single one for a row that names no
/// mode and so binds in whatever keymap is current.
fn bind_lines(binding: &Binding, clear_line: Option<&str>) -> Result<Vec<String>, RenderFault> {
    let key = keys::to_readline(&binding.key)
        .map_err(|token| RenderFault(format!("unknown key token {:?}", token.0)))?;
    let action = binding
        .action()
        .map_err(|fault_kind| RenderFault(fault_kind.to_string()))?;
    let clear_line = clear_line_for(&action, clear_line)?;

    let modes = binding.modes();
    if modes.is_empty() {
        return Ok(vec![bind_line(None, &key, &action, clear_line)?]);
    }
    modes
        .iter()
        .map(|mode| bind_line(Some(mode), &key, &action, clear_line))
        .collect()
}

/// A row that types text clears the line first, so it needs the macro that
/// does; the other row kinds never ask for one.
fn clear_line_for<'a>(
    action: &Action<'_>,
    configured: Option<&'a str>,
) -> Result<Option<&'a str>, RenderFault> {
    if !matches!(action, Action::Insert(_) | Action::Run(_)) {
        return Ok(None);
    }
    configured.map(Some).ok_or_else(|| {
        RenderFault(
            "types over the line, which needs the clear-line macro no `clear_line` \
             under [render.bash] names"
                .to_string(),
        )
    })
}

fn bind_line(
    mode: Option<&str>,
    key: &str,
    action: &Action<'_>,
    clear_line: Option<&str>,
) -> Result<String, RenderFault> {
    let flag = match mode {
        Some(mode) => format!("-m {mode} "),
        None => String::new(),
    };
    let executes = matches!(action, Action::Function(_));
    let value = match action {
        Action::Command(command) => (*command).to_string(),
        Action::Function(name) => format!("\"{}\"", escape_for_readline(name)?),
        Action::Insert(text) | Action::Run(text) | Action::Macro(text) => {
            format!("\"{}\"", macro_body(mode, action, text, clear_line)?)
        }
    };
    let payload = format!("\"{key}\": {value}");
    Ok(format!(
        "builtin bind {flag}{}{}",
        if executes { "-x " } else { "" },
        single_quote(&payload)
    ))
}

/// A typed macro clears the line first, and in vi's command mode enters
/// insert mode before it types. `Run` ends with the carriage return that
/// submits the line. A raw `Macro` is emitted verbatim, so a row that needs
/// one writes its own leading `i` where vi's command mode needs it.
fn macro_body(
    mode: Option<&str>,
    action: &Action<'_>,
    text: &str,
    clear_line: Option<&str>,
) -> Result<String, RenderFault> {
    if let Action::Macro(_) = action {
        return Ok(text.to_string());
    }
    let mut body = String::new();
    if mode == Some(COMMAND_MODE) {
        body.push('i');
    }
    let Some(clear_line) = clear_line else {
        unreachable!("a typed row without a clear-line macro is refused before here")
    };
    body.push_str(clear_line);
    body.push_str(&escape_for_readline(text)?);
    if let Action::Run(_) = action {
        body.push_str("\\r");
    }
    Ok(body)
}

/// Escape text for the inside of readline's double quotes. A carriage return
/// is refused: it is what `run` appends, and a row that carries its own would
/// submit the line somewhere the table does not say.
fn escape_for_readline(text: &str) -> Result<String, RenderFault> {
    let mut out = String::new();
    for character in text.chars() {
        match character {
            '\r' => {
                return Err(RenderFault(
                    "a carriage return belongs to `run`, not to the text it types".to_string(),
                ));
            }
            '\n' => out.push_str("\\n"),
            '"' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            _ => out.push(character),
        }
    }
    Ok(out)
}

/// Wrap the payload in the single quotes bash's `bind` argument uses,
/// splicing any single quote of its own back in, which is how a quoted
/// argument inside a bound command survives.
fn single_quote(payload: &str) -> String {
    format!("'{}'", payload.replace('\'', "'\\''"))
}

fn fault(binding: &Binding, reason: &str) -> RenderFault {
    RenderFault(format!("binding {:?}: {reason}", binding.key))
}

#[cfg(test)]
mod tests;
