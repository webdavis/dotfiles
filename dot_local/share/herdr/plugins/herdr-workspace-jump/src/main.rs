//! Workspace Jump: create-or-focus a herdr workspace by label.
//!
//! Bound to the quick-jump chords as one `plugin_action` per workspace. A
//! keybinding passes no arguments, so each action's argv carries the label and
//! the working directory; herdr does not run an action through a shell, so `~`
//! is expanded here rather than by the shell.
//!
//! Usage: herdr-workspace-jump <label> <cwd>

mod cli;
mod jump;
mod protocol;
mod socket;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::CliWorkspaceDirectory;
use jump::{Jump, jump};
use protocol::JumpError;
use socket::{DEADLINE, SocketWorkspaceDirectory};

/// Expand a leading `~`, which herdr's argv-only action commands cannot do.
fn expand_tilde(raw: &str, home: Option<&str>) -> Result<PathBuf, String> {
    let tail = match raw.strip_prefix('~') {
        None => return Ok(PathBuf::from(raw)),
        Some(tail) => tail,
    };
    if !tail.is_empty() && !tail.starts_with('/') {
        return Err(format!("cannot expand another user's home in {raw}"));
    }
    let home = home
        .filter(|home| !home.is_empty())
        .ok_or_else(|| format!("cannot expand {raw}: HOME is unset"))?;
    Ok(PathBuf::from(home).join(tail.trim_start_matches('/')))
}

/// Try the socket, then the CLI.
///
/// The whole jump is retried rather than the failed call, because the socket
/// may have failed before it listed anything. That is safe in both directions:
/// a create that failed did not create, and a create whose answer was lost is
/// found by the second list and focused instead of duplicated.
fn run(
    socket_path: Option<String>,
    herdr_binary: Option<String>,
    label: &str,
    cwd: &str,
) -> Result<Jump, JumpError> {
    let over_socket = match socket_path.filter(|path| !path.is_empty()) {
        Some(path) => attempt_socket(Path::new(&path), label, cwd),
        None => Err(JumpError::Unreachable(
            "HERDR_SOCKET_PATH is unset".to_string(),
        )),
    };
    let socket_failure = match over_socket {
        Ok(outcome) => return Ok(outcome),
        Err(failure) => failure,
    };
    let mut directory = CliWorkspaceDirectory::using(herdr_binary);
    jump(&mut directory, label, cwd).map_err(|cli_failure| JumpError::BothPathsFailed {
        cli: cli_failure.to_string(),
        socket: socket_failure.to_string(),
    })
}

fn attempt_socket(path: &Path, label: &str, cwd: &str) -> Result<Jump, JumpError> {
    let mut directory = SocketWorkspaceDirectory::connect(path, DEADLINE)?;
    jump(&mut directory, label, cwd)
}

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
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_joins_a_leading_tilde_onto_home() {
        assert_eq!(
            expand_tilde("~/workspaces/Ivy", Some("/Users/me")),
            Ok(PathBuf::from("/Users/me/workspaces/Ivy"))
        );
        assert_eq!(
            expand_tilde("~", Some("/Users/me")),
            Ok(PathBuf::from("/Users/me"))
        );
    }

    #[test]
    fn expand_tilde_leaves_an_absolute_or_relative_path_alone() {
        assert_eq!(
            expand_tilde("/opt/project", Some("/Users/me")),
            Ok(PathBuf::from("/opt/project"))
        );
        assert_eq!(
            expand_tilde("relative/path", Some("/Users/me")),
            Ok(PathBuf::from("relative/path"))
        );
        // A tilde anywhere but the front is an ordinary character.
        assert_eq!(
            expand_tilde("/opt/~backup", Some("/Users/me")),
            Ok(PathBuf::from("/opt/~backup"))
        );
    }

    #[test]
    fn expand_tilde_refuses_another_users_home_and_a_missing_home() {
        assert!(expand_tilde("~other/project", Some("/Users/me")).is_err());
        assert!(expand_tilde("~/project", None).is_err());
        assert!(expand_tilde("~/project", Some("")).is_err());
    }

    /// Answer `replies` on one connection, so `run` can be driven end to end.
    /// Returns the socket path; the directory is cleaned by the caller.
    fn one_shot_herdr(name: &str, replies: Vec<String>) -> PathBuf {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;

        let directory = std::env::temp_dir().join(format!("hwj-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        if let Err(error) = std::fs::create_dir_all(&directory) {
            panic!("could not make the socket directory: {error}");
        }
        let path = directory.join("herdr.sock");
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) => panic!("could not bind the fake socket: {error}"),
        };
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            for reply in replies {
                let mut request = String::new();
                match reader.read_line(&mut request) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                if reader
                    .get_mut()
                    .write_all(format!("{reply}\n").as_bytes())
                    .is_err()
                {
                    return;
                }
            }
        });
        path
    }

    #[test]
    fn run_completes_a_jump_over_the_socket_without_touching_the_cli() {
        let list =
            r#"{"id":"req_1","result":{"workspaces":[{"workspace_id":"w11","label":"dotfiles"}]}}"#;
        let ok = r#"{"id":"req_2","result":{"type":"ok"}}"#;
        let path = one_shot_herdr("run-ok", vec![list.to_string(), ok.to_string()]);

        // The CLI binary does not exist, so a fallback would fail the assertion.
        let outcome = run(
            Some(path.to_string_lossy().to_string()),
            Some("/nonexistent/herdr-binary".to_string()),
            "dotfiles",
            "/tmp",
        );

        assert_eq!(outcome, Ok(Jump::Focused("w11".to_string())));
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn run_falls_back_to_the_cli_when_the_socket_path_is_absent() {
        // Both paths are dead, so the surviving error proves which one answered
        // last: the CLI failure, carrying the socket failure behind it.
        let failure = run(
            None,
            Some("/nonexistent/herdr-binary".to_string()),
            "dotfiles",
            "/tmp",
        );
        let rendered = match failure {
            Err(error) => error.to_string(),
            Ok(outcome) => panic!("expected both paths to fail, got {outcome:?}"),
        };
        assert!(rendered.contains("/nonexistent/herdr-binary"), "{rendered}");
        assert!(
            rendered.contains("HERDR_SOCKET_PATH is unset"),
            "{rendered}"
        );
        // Exactly one prefix per failure, never the wrapper's stacked on top of
        // the wrapped one's. Two failures are named, so two prefixes.
        assert_eq!(
            rendered.matches("herdr unreachable").count(),
            2,
            "{rendered}"
        );
    }

    #[test]
    fn run_falls_back_to_the_cli_when_the_socket_cannot_be_opened() {
        let absent = std::env::temp_dir().join("hwj-run-no-socket");
        let _ = std::fs::remove_file(&absent);
        let failure = run(
            Some(absent.to_string_lossy().to_string()),
            Some("/nonexistent/herdr-binary".to_string()),
            "dotfiles",
            "/tmp",
        );
        let rendered = match failure {
            Err(error) => error.to_string(),
            Ok(outcome) => panic!("expected both paths to fail, got {outcome:?}"),
        };
        assert!(rendered.contains("hwj-run-no-socket"), "{rendered}");
    }
}
