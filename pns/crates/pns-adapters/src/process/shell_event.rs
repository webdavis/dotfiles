use pns_domain::EventArgs;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub fn spawn_shell_event(event: &EventArgs) -> io::Result<()> {
    spawn(std::env::current_exe()?, event)
}

fn spawn(binary: std::path::PathBuf, event: &EventArgs) -> io::Result<()> {
    let mut command = Command::new(binary);
    command.args([
        "--agent",
        &event.agent,
        "--state",
        &event.state,
        "--project",
        &event.project,
        "--detail",
        &event.detail,
        "--pane",
        &event.pane,
    ]);
    if event.long_running {
        command.arg("--long-running");
    }
    // Marker removal already finished. This is the existing producer route,
    // off the prompt's job table and streams, with its existing delivery bounds.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests;
