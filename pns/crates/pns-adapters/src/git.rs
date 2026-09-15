use crate::{PROBE_READ_MAX, run_bounded};
use std::path::Path;
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

/// The checkout an event fired in: the repository it belongs to and the branch
/// it is on.
///
/// Both empty outside a repository, which is a state and never an error: the
/// caller falls back to the working directory's own name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Checkout {
    /// The REPOSITORY, not the worktree. A linked worktree's directory is
    /// named for its branch, so the last segment of `cwd` answers
    /// `feat-pns-sender-header` where the operator's question is about
    /// `dotfiles`.
    pub repository: String,
    /// The branch checked out, or the worktree's own directory name on a
    /// detached head, where there is no branch to name.
    pub branch: String,
}

/// What git says about `cwd`, in ONE spawn: `rev-parse` answers every option
/// it is given, one line each, in the order they were asked for. Bounded like
/// every other spawn here, and a failed read is an empty answer rather than a
/// held notification.
///
/// ONE SPAWN MEANS ONE VERDICT. `rev-parse` refuses an unborn HEAD (a
/// repository with no commit yet) and that refusal takes the whole answer with
/// it, so such a checkout reports nothing and the caller falls back to the
/// working directory's own name. That fallback is the right answer there: a
/// repository without a commit cannot have a linked worktree, so its
/// directory IS its name.
pub fn git_checkout(cwd: &str) -> Checkout {
    if cwd.is_empty() || !Path::new(cwd).is_dir() {
        return Checkout::default();
    }
    let mut command = Command::new("git");
    command.args([
        "-C",
        cwd,
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
        "--show-toplevel",
        "--abbrev-ref",
        "HEAD",
    ]);
    run_bounded(command, None, GIT_DEADLINE, PROBE_READ_MAX)
        .map(|answer| checkout_of(&answer))
        .unwrap_or_default()
}

/// git's three answers, read in the order they were asked for: the common
/// directory, the top of this worktree, and the head.
fn checkout_of(answer: &str) -> Checkout {
    let mut lines = answer.lines().map(str::trim);
    let common = lines.next().unwrap_or_default();
    let worktree = lines.next().unwrap_or_default();
    let head = lines.next().unwrap_or_default();
    Checkout {
        repository: repository_of(common),
        // `--abbrev-ref HEAD` answers the literal `HEAD` on a detached head,
        // which no branch can be named, so the worktree's own name takes the
        // slot rather than leaving it empty.
        branch: match head {
            "" | "HEAD" => basename(worktree),
            branch => branch.to_string(),
        },
    }
}

/// The repository a common directory belongs to.
///
/// `<repository>/.git` is the ordinary answer, in the main checkout and in
/// every linked worktree alike, so the directory HOLDING it is the name. A
/// bare repository answers a directory that IS the repository, and naming its
/// parent there would answer with a path segment nobody calls the project.
fn repository_of(common: &str) -> String {
    let path = Path::new(common);
    if path.file_name().is_some_and(|name| name == ".git") {
        return path.parent().map(basename_of).unwrap_or_default();
    }
    basename_of(path)
}

fn basename(path: &str) -> String {
    basename_of(Path::new(path))
}

fn basename_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
