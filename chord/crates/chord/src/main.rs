//! `chord`: shell key bindings are one shell-agnostic table, and this tool
//! renders that table into each shell's native binding syntax.
//!
//! `chord render <shell> --table <file>` writes the rendering to standard
//! output. `chord check <shell> --table <file> --against <file>` proves a
//! generated file still matches its table, which is what keeps a hand edit
//! to the generated file from going unnoticed.

use std::process::ExitCode;

mod check;
mod keys;
mod render;
mod table;

const USAGE: &str = "usage: chord render <shell> --table <file>\n       chord check <shell> --table <file> --against <file>";

/// The rendering and the file disagree.
const DIFFERS: u8 = 1;

/// Argv, a table or a shell this tool does not serve.
const REFUSED: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["render", shell, "--table", table] => render(shell, table),
        ["check", shell, "--table", table, "--against", against] => check(shell, table, against),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(REFUSED)
        }
    }
}

fn render(shell: &str, table_path: &str) -> ExitCode {
    match rendering(shell, table_path) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(refusal) => refuse(&refusal),
    }
}

fn check(shell: &str, table_path: &str, against_path: &str) -> ExitCode {
    let rendered = match rendering(shell, table_path) {
        Ok(text) => text,
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
    eprintln!("{against_path} is not what the table renders; run `just chord-render`");
    ExitCode::from(DIFFERS)
}

fn rendering(shell: &str, table_path: &str) -> Result<String, String> {
    let renderer = render::for_shell(shell)
        .ok_or_else(|| format!("chord does not render bindings for the shell {shell:?}"))?;
    let text = std::fs::read_to_string(table_path)
        .map_err(|fault| format!("cannot read {table_path}: {fault}"))?;
    let table: table::Table =
        toml::from_str(&text).map_err(|fault| format!("{table_path}: {fault}"))?;
    renderer.render(&table).map_err(|fault| fault.to_string())
}

fn refuse(reason: &str) -> ExitCode {
    eprintln!("{reason}");
    ExitCode::from(REFUSED)
}
