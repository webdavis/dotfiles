use pns_domain::EventArgs;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub fn spawn_shell_event(event: &EventArgs, elapsed: u64) -> io::Result<()> {
    spawn(std::env::current_exe()?, event, elapsed)
}

fn spawn(binary: std::path::PathBuf, event: &EventArgs, elapsed: u64) -> io::Result<()> {
    let mut command = Command::new(binary);
    // `--elapsed` lets the spawned `pns send` derive `long_running` itself,
    // the same way every other producer does; there is no flag to carry a
    // precomputed boolean any more.
    command.args([
        "send",
        "--producer",
        &event.agent,
        "--state",
        &event.state,
        "--project",
        &event.project,
        "--detail",
        &event.detail,
        "--pane",
        &event.pane,
        "--elapsed",
        &format!("{elapsed}s"),
    ]);
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
