//! Workspace Jump: create-or-focus a herdr workspace by label.
//!
//! Bound to the quick-jump chords as one `plugin_action` per workspace. A
//! keybinding passes no arguments, so each action's argv carries the label and
//! the working directory; herdr does not run an action through a shell, so `~`
//! is expanded here rather than by the shell.
//!
//! Usage: `herdr-workspace-jump <label> <cwd>`

mod path;
mod run;

use std::env;
use std::process::ExitCode;

use path::expand_tilde;
use run::run;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [label, raw_cwd] = arguments.as_slice() else {
        eprintln!("herdr-workspace-jump: usage: herdr-workspace-jump <label> <cwd>");
        return ExitCode::from(2);
    };
    let cwd = match expand_tilde(raw_cwd, env::var("HOME").ok().as_deref()) {
        Ok(cwd) => cwd,
        Err(reason) => {
            eprintln!("herdr-workspace-jump: {reason}");
            return ExitCode::FAILURE;
        }
    };
    match run(
        env::var("HERDR_SOCKET_PATH").ok(),
        env::var("HERDR_BIN_PATH").ok(),
        label,
        &cwd.to_string_lossy(),
    ) {
        Ok(_) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("herdr-workspace-jump: could not jump to {label}: {failure}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "tests/socket_server.rs"]
mod socket_server;

#[cfg(test)]
#[path = "tests/cli_command.rs"]
mod cli_command;
