//! `chord`: shell key bindings are one shell-agnostic table, and this tool
//! renders that table into each shell's native binding syntax.
//!
//! `chord render <target> --table <file>` writes the rendering to standard
//! output. `chord check <target> --table <file> --against <file>` proves a
//! generated file still matches its table, which is what keeps a hand edit
//! to the generated file from going unnoticed.
//!
//! The targets are `bash`, readline `bind` calls, and `menu`, one
//! tab-separated record per row for the shell's binding picker.

use std::process::ExitCode;

mod check;
mod keys;
mod render;
mod table;

const USAGE: &str = "usage: chord render <target> --table <file>\n       chord check <target> --table <file> --against <file>\n       <target> is bash or menu";

/// The rendering and the file disagree.
const DIFFERS: u8 = 1;

/// Argv, a table or a target this tool does not serve.
const REFUSED: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["render", target, "--table", table] => render(target, table),
        ["check", target, "--table", table, "--against", against] => check(target, table, against),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(REFUSED)
        }
    }
}

fn render(target: &str, table_path: &str) -> ExitCode {
    match rendering(target, table_path) {
        Ok((text, _)) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(refusal) => refuse(&refusal),
    }
}

fn check(target: &str, table_path: &str, against_path: &str) -> ExitCode {
    let (rendered, regenerate) = match rendering(target, table_path) {
        Ok(rendering) => rendering,
        Err(refusal) => return refuse(&refusal),
    };
    let on_disk = match std::fs::read_to_string(against_path) {
        Ok(text) => text,
        Err(fault) => return refuse(&format!("cannot read {against_path}: {fault}")),
    };
    let difference = check::difference(&on_disk, &rendered, against_path);
    if difference.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprint!("{difference}");
    eprintln!("{}", mismatch(against_path, regenerate.as_deref()));
    ExitCode::from(DIFFERS)
}

/// What a failed check tells the reader. Only the table knows the command
/// that regenerates the file, so a table that names none says what is wrong
/// without naming a tool the reader may not have.
fn mismatch(against_path: &str, regenerate: Option<&str>) -> String {
    let remedy = match regenerate {
        Some(command) => format!("run `{command}`"),
        None => "render the table again".to_string(),
    };
    format!("{against_path} is not what the table renders; {remedy}")
}

/// The rendering, and the command the table names for regenerating it,
/// which is what a failed check needs beside the diff.
fn rendering(target: &str, table_path: &str) -> Result<(String, Option<String>), String> {
    let renderer = render::for_target(target)
        .ok_or_else(|| format!("chord does not render the target {target:?}"))?;
    let text = std::fs::read_to_string(table_path)
        .map_err(|fault| format!("cannot read {table_path}: {fault}"))?;
    let table: table::Table =
        toml::from_str(&text).map_err(|fault| format!("{table_path}: {fault}"))?;
    let rendered = renderer.render(&table).map_err(|fault| fault.to_string())?;
    Ok((rendered, table.render.regenerate))
}

fn refuse(reason: &str) -> ExitCode {
    eprintln!("{reason}");
    ExitCode::from(REFUSED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regenerate_command_the_table_names_is_what_the_reader_is_told_to_run() {
        assert_eq!(
            mismatch("dot_bash_bindings", Some("just chord-render")),
            "dot_bash_bindings is not what the table renders; run `just chord-render`"
        );
    }

    #[test]
    fn a_table_naming_no_regenerate_command_still_says_what_is_wrong_and_names_no_tool() {
        let message = mismatch("bindings.sh", None);
        assert_eq!(
            message,
            "bindings.sh is not what the table renders; render the table again"
        );
        assert!(!message.contains('`'), "{message}");
    }
}
