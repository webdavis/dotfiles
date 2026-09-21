//! Bash: readline `bind` calls, in the style of the file this replaces.

use super::{RenderFault, ShellRenderer};
use crate::keys;
use crate::table::{Action, Binding, Group, Table};

/// Kept in the output because the file it generates is still shell that
/// shellcheck reads and the operator edits the table beside.
const HEADER: &str = "\
# vi:filetype=sh shiftwidth=2 softtabstop=2 tabstop=8:

# shellcheck shell=bash

# Disable SC2016 (which warns about expressions in single quotes):
# shellcheck disable=SC2016

# GENERATED FILE: `chord render bash` over `dot_config/chord/bindings.toml`.
# Edit the table and regenerate with `just chord-render`; a hand edit here
# fails `just test-rust`.
#
# The shell functions these bindings call, and the `\\C-x0` helper macro, live
# in ~/.bash_bindings_functions, which ~/.bashrc sources first.
";

/// The keymap whose bindings must first leave vi's command mode, which is
/// what the leading `i` in a command-mode macro does.
const COMMAND_MODE: &str = "vi-command";

/// Clears the line before a macro types over it. Bound in the functions file.
const CLEAR_LINE: &str = "\\C-x0";

pub struct Bash;

impl ShellRenderer for Bash {
    fn render(&self, table: &Table) -> Result<String, RenderFault> {
        let mut out = String::from(HEADER);
        for group in &table.group {
            render_group(group, &mut out)?;
        }
        Ok(out)
    }
}

fn render_group(group: &Group, out: &mut String) -> Result<(), RenderFault> {
    out.push_str(&format!("\n# --- {} ---\n", group.name));
    push_comment(group.description.as_deref(), out);
    for binding in &group.binding {
        out.push('\n');
        push_comment(binding.description.as_deref(), out);
        for line in render_binding(binding)? {
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

/// One `builtin bind` line per mode, or a single one for a row that names no
/// mode and so binds in whatever keymap is current.
fn render_binding(binding: &Binding) -> Result<Vec<String>, RenderFault> {
    let key = keys::to_readline(&binding.key)
        .map_err(|token| fault(binding, &format!("unknown key token {:?}", token.0)))?;
    let action = binding
        .action()
        .map_err(|fault_kind| fault(binding, &format!("{fault_kind:?}")))?;

    let modes = binding.modes();
    if modes.is_empty() {
        return Ok(vec![bind_line(None, &key, &action)?]);
    }
    modes
        .iter()
        .map(|mode| bind_line(Some(mode), &key, &action))
        .collect()
}

fn bind_line(mode: Option<&str>, key: &str, action: &Action<'_>) -> Result<String, RenderFault> {
    let flag = match mode {
        Some(mode) => format!("-m {mode} "),
        None => String::new(),
    };
    let executes = matches!(action, Action::Function(_));
    let value = match action {
        Action::Command(command) => (*command).to_string(),
        Action::Function(name) => format!("\"{}\"", escape_for_readline(name)?),
        Action::Insert(text) | Action::Run(text) | Action::Macro(text) => {
            format!("\"{}\"", macro_body(mode, action, text)?)
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
fn macro_body(mode: Option<&str>, action: &Action<'_>, text: &str) -> Result<String, RenderFault> {
    if let Action::Macro(_) = action {
        return Ok(text.to_string());
    }
    let mut body = String::new();
    if mode == Some(COMMAND_MODE) {
        body.push('i');
    }
    match action {
        Action::Insert(_) => {
            body.push_str(CLEAR_LINE);
            body.push_str(&escape_for_readline(text)?);
        }
        Action::Run(_) => {
            body.push_str(CLEAR_LINE);
            body.push_str(&escape_for_readline(text)?);
            body.push_str("\\r");
        }
        _ => unreachable!("only typed macros reach macro_body"),
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
