use crate::{PROBE_READ_MAX, run_bounded};
use std::process::Command;
use std::time::Duration;

/// The branch the work happened on, or none. Bounded like every other spawn:
/// a wedged git must not hold a notification.
pub fn git_branch(cwd: &str) -> String {
    if cwd.is_empty() || !std::path::Path::new(cwd).is_dir() {
        return String::new();
    }
    let mut command = Command::new("git");
    command.args(["-C", cwd, "branch", "--show-current"]);
    run_bounded(command, None, GIT_DEADLINE, PROBE_READ_MAX)
        .map(|branch| branch.trim().to_string())
        .unwrap_or_default()
}
/// A branch lookup is a local read; anything slower than this is a wedged
/// repository, not an answer worth waiting for.
const GIT_DEADLINE: Duration = Duration::from_secs(5);
