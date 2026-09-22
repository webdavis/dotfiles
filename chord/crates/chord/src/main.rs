//! `chord`: shell key bindings are one shell-agnostic table, and this tool
//! renders that table into each shell's native binding syntax.
//!
//! `chord render <target> --table <file>` writes the rendering to the file
//! the table's `[render.<target>] output` names, or to standard output when
//! it names none. `chord check <target> --table <file>` proves that file
//! still matches its table, which is what keeps a hand edit to a generated
//! file from going unnoticed; `--against <file>` compares some other file
//! instead. A relative path, in the table or on the command line, is read
//! from the working directory.
//!
//! The targets are `bash`, readline `bind` calls, and `menu`, one
//! tab-separated record per row for the shell's binding picker.

use std::process::ExitCode;

mod check;
mod keys;
mod output;
mod render;
mod table;

const USAGE: &str = "usage: chord render <target> --table <file>\n       chord check <target> --table <file> [--against <file>]\n       <target> is bash or menu";

/// The rendering and the file disagree.
const DIFFERS: u8 = 1;

/// Argv, a table or a target this tool does not serve.
const REFUSED: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["render", target, "--table", table] => render(target, table),
        ["check", target, "--table", table] => check(target, table, None),
        ["check", target, "--table", table, "--against", against] => {
            check(target, table, Some(against))
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(REFUSED)
        }
    }
}

fn render(target: &str, table_path: &str) -> ExitCode {
    let rendering = match rendering(target, table_path) {
        Ok(rendering) => rendering,
        Err(refusal) => return refuse(&refusal),
    };
    match rendering.output {
        None => {
            print!("{}", rendering.text);
            ExitCode::SUCCESS
        }
        Some(path) => match output::write(&path, &rendering.text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(refusal) => refuse(&refusal),
        },
    }
}

fn check(target: &str, table_path: &str, against_path: Option<&str>) -> ExitCode {
    let rendering = match rendering(target, table_path) {
        Ok(rendering) => rendering,
        Err(refusal) => return refuse(&refusal),
    };
    let compared = match comparison(target, against_path, rendering.output.as_deref()) {
        Ok(path) => path.to_string(),
        Err(refusal) => return refuse(&refusal),
    };
    let on_disk = match std::fs::read_to_string(&compared) {
        Ok(text) => text,
        Err(fault) => return refuse(&format!("cannot read {compared}: {fault}")),
    };
    let difference = check::difference(&on_disk, &rendering.text, &compared);
    if difference.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprint!("{difference}");
    eprintln!("{}", mismatch(&compared, rendering.regenerate.as_deref()));
    ExitCode::from(DIFFERS)
}

/// The file a check reads. An explicit `--against` wins, since comparing
/// against some other file is a caller's business; otherwise the target
/// compares the file it writes.
fn comparison<'a>(
    target: &str,
    against_path: Option<&'a str>,
    output: Option<&'a str>,
) -> Result<&'a str, String> {
    against_path.or(output).ok_or_else(|| {
        format!("the table names no output for {target}, so a check needs --against <file>")
    })
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

/// A rendering, with the two things the table says about it that the
/// rendering itself cannot carry.
struct Rendering {
    text: String,
    /// The file to write, or `None` for standard output.
    output: Option<String>,
    /// The command a failed check names beside the diff.
    regenerate: Option<String>,
}

fn rendering(target: &str, table_path: &str) -> Result<Rendering, String> {
    let renderer = render::for_target(target)
        .ok_or_else(|| format!("chord does not render the target {target:?}"))?;
    let text = std::fs::read_to_string(table_path)
        .map_err(|fault| format!("cannot read {table_path}: {fault}"))?;
    let table: table::Table =
        toml::from_str(&text).map_err(|fault| format!("{table_path}: {fault}"))?;
    Ok(Rendering {
        text: renderer.render(&table).map_err(|fault| fault.to_string())?,
        output: renderer.output(&table).map(str::to_string),
        regenerate: table.render.regenerate,
    })
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

#[cfg(test)]
mod comparison_tests {
    use super::*;

    #[test]
    fn an_explicit_against_path_wins_over_the_tables_output() {
        assert_eq!(
            comparison("bash", Some("other.sh"), Some("bindings.sh")),
            Ok("other.sh")
        );
    }

    #[test]
    fn the_tables_output_is_what_a_check_compares_when_no_path_is_given() {
        assert_eq!(
            comparison("bash", None, Some("bindings.sh")),
            Ok("bindings.sh")
        );
    }

    #[test]
    fn a_check_with_neither_a_path_nor_an_output_says_what_is_missing() {
        let refusal = comparison("bash", None, None).expect_err("a check needs a file");
        assert!(refusal.contains("--against"), "{refusal}");
        assert!(refusal.contains("bash"), "{refusal}");
    }
}
