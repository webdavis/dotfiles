//! The composition root: config in, one framed page out.
//!
//! morning NEVER CHANGES ANYTHING. It reads files the config names and runs
//! the commands the config names, and that is the whole of its contact with
//! the machine. It applies nothing, merges nothing and edits nothing.

mod brief;

use morning_adapters::Config;
use morning_adapters::style::Paint;
use std::io::IsTerminal;
use std::path::Path;

/// What one run produced.
pub struct Response {
    pub stdout: String,
    pub stderr: String,
    pub exit: u8,
}

const USAGE: &str = "usage: morning [--config <path>] [--no-color]\n\n\
Prints one page: the last apply, the applies the ledger owes, the open pull\n\
requests, the newest overnight recap, the operator's own items and today's\n\
tasks. It reads and prints; it changes nothing.\n\n\
--no-color forces a plain page, with no escape sequences, whatever the\n\
destination is.\n";

/// Runs morning. `config_path` is the default location; `--config` overrides it.
pub fn run(args: &[String], config_path: &Path, home: &Path) -> Response {
    let (config_path, forced_plain) = match parse_args(args, config_path) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return page(USAGE.to_string()),
        Err(message) => {
            return Response {
                stdout: String::new(),
                stderr: format!("morning: {message}\n{USAGE}"),
                exit: 2,
            };
        }
    };
    let paint = Paint::decide(forced_plain, std::io::stdout().is_terminal());
    match Config::load(&config_path) {
        Ok(config) => page(brief::compose(&config, home, paint)),
        Err(message) => Response {
            stdout: String::new(),
            stderr: format!("morning: {message}\n"),
            exit: 1,
        },
    }
}

fn page(stdout: String) -> Response {
    Response {
        stdout,
        stderr: String::new(),
        exit: 0,
    }
}

/// The config file to read and whether `--no-color` was typed, or `None` for
/// the usage text. An unknown argument is an error rather than a silent
/// fall-through to help.
fn parse_args(
    args: &[String],
    default: &Path,
) -> Result<Option<(std::path::PathBuf, bool)>, String> {
    let mut chosen = default.to_path_buf();
    let mut forced_plain = false;
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--no-color" => forced_plain = true,
            "--config" => match arguments.next() {
                Some(path) => chosen = std::path::PathBuf::from(path),
                None => return Err("--config needs a path".to_string()),
            },
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Some((chosen, forced_plain)))
}

#[cfg(test)]
mod tests;
