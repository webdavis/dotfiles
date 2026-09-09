use pns_adapters::marker_files::{begin_shell, end_shell};
use pns_domain::shell_event;

mod parse;
use parse::{Action, parse};

pub fn shell_mode(argv: &[String]) -> i32 {
    let action = match parse(argv) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("pns: shell {error}");
            return 2;
        }
    };
    let result = match action {
        Action::Begin { pid, command } => begin_shell(pid, command),
        Action::End {
            pid,
            command,
            exit_code,
            elapsed,
        } => {
            // Clearing is synchronous and precedes every path that can notify.
            // A fresh begin after this command returns cannot be erased later.
            end_shell(pid).and_then(|()| {
                let cwd = std::env::var("PWD").unwrap_or_default();
                let project = cwd.rsplit('/').next().unwrap_or_default().to_owned();
                let pane = std::env::var("HERDR_PANE_ID").unwrap_or_default();
                match shell_event(command, exit_code, elapsed, project, pane) {
                    Some(event) => pns_adapters::spawn_shell_event(&event),
                    None => Ok(()),
                }
            })
        }
    };
    if let Err(error) = result {
        eprintln!("pns: shell {error}");
        return 1;
    }
    0
}
