use crate::{EventArgs, elapsed_event};

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

pub fn shell_event(
    command: &str,
    exit_code: u8,
    elapsed: u64,
    project: String,
    pane: String,
) -> Option<EventArgs> {
    if shell_is_interactive(command) {
        return None;
    }
    // Bash's old ${name%% *} names the command, never its arguments. Preserve
    // that literal-space boundary rather than interpreting a shell program.
    let name = command.split(' ').next().unwrap_or_default();
    let mut event = elapsed_event(
        EventArgs {
            agent: "shell".into(),
            state: if exit_code == 0 { "done" } else { "failed" }.into(),
            project,
            pane,
            detail: name.into(),
            ..EventArgs::default()
        },
        elapsed,
    )?;
    // A blank first-command history still has the same parenthesized detail.
    event.detail = if exit_code == 0 {
        format!("{name} ({elapsed}s)")
    } else {
        format!("{name} ({elapsed}s, exit {exit_code})")
    };
    Some(event)
}

#[cfg(test)]
mod tests;
