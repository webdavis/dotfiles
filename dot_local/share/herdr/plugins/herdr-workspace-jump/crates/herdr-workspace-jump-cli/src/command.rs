use std::env;
use std::path::{Path, PathBuf};

use herdr_workspace_jump_adapters::{FileWorkspaceHistory, state_file};
use herdr_workspace_jump_application::record;
use herdr_workspace_jump_protocol::parse_focused_id;

use crate::{last_workspace, path::expand_tilde, run};

pub(crate) const USAGE: &str = "usage: herdr-workspace-jump jump <label> <cwd> | herdr-workspace-jump last-workspace | herdr-workspace-jump record";

pub(crate) enum CommandError {
    Usage,
    Failed(String),
}

fn history_path() -> PathBuf {
    state_file(
        env::var("HERDR_PLUGIN_STATE_DIR").ok().as_deref(),
        env::var("HOME").ok().as_deref(),
    )
}

pub(crate) fn record_at(state: &Path, event: &str) {
    if let Some(new_id) = parse_focused_id(event) {
        record(&mut FileWorkspaceHistory::at(state), &new_id);
    }
}

pub(crate) fn execute(arguments: &[String]) -> Result<(), CommandError> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["jump", label, raw_cwd] => {
            let cwd = expand_tilde(raw_cwd, env::var("HOME").ok().as_deref())
                .map_err(CommandError::Failed)?;
            run::run(
                env::var("HERDR_SOCKET_PATH").ok(),
                env::var("HERDR_BIN_PATH").ok(),
                label,
                &cwd.to_string_lossy(),
            )
            .map(|_| ())
            .map_err(|failure| {
                CommandError::Failed(format!("could not jump to {label}: {failure}"))
            })
        }
        ["last-workspace"] => last_workspace::run(
            env::var("HERDR_SOCKET_PATH").ok(),
            env::var("HERDR_BIN_PATH").ok(),
            &mut FileWorkspaceHistory::at(&history_path()),
        )
        .map(|_| ())
        .map_err(|failure| {
            CommandError::Failed(format!(
                "could not toggle to the previous workspace: {failure}"
            ))
        }),
        ["record"] => {
            record_at(
                &history_path(),
                &env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default(),
            );
            Ok(())
        }
        _ => Err(CommandError::Usage),
    }
}

#[cfg(test)]
mod tests;
