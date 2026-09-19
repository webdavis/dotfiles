use crate::EventArgs;

pub fn shell_is_interactive(command: &str) -> bool {
    [
        "vim", "nvim", "less", "man", "top", "btop", "ssh", "herdr", "claude", "hermes", "codex",
        "fzf",
    ]
    .iter()
    .any(|name| {
        command
            .strip_prefix(name)
            .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
    })
}

/// The event a finished shell command notifies as, when it ran long enough to
/// earn one, or `None` for a command under thirty seconds.
///
/// THE TIER ISN'T DECIDED HERE. This crosses a process boundary to a spawned
/// `pns send`, which re-derives `long_running` from `--elapsed` the same way
/// every other producer does, so the raw `elapsed` seconds travel alongside
/// this event rather than a precomputed boolean.
pub fn shell_event(
    command: &str,
    exit_code: u8,
    elapsed: u64,
    project: String,
    pane: String,
) -> Option<EventArgs> {
    if shell_is_interactive(command) || elapsed < 30 {
        return None;
    }
    // Bash's old ${name%% *} names the command, never its arguments. Preserve
    // that literal-space boundary rather than interpreting a shell program.
    let name = command.split(' ').next().unwrap_or_default();
    let detail = if exit_code == 0 {
        name.to_string()
    } else {
        format!("{name}, exit {exit_code}")
    };
    Some(EventArgs {
        agent: "shell".into(),
        state: if exit_code == 0 { "done" } else { "failed" }.into(),
        project,
        pane,
        detail,
        ..EventArgs::default()
    })
}

#[cfg(test)]
mod tests;
