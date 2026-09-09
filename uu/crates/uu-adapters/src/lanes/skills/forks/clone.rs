use crate::CommandRunner;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
type Comparison = Result<String, (&'static str, String)>;
pub(super) fn inspect(
    parent: &Path,
    fields: &[&str; 3],
    runner: &dyn CommandRunner,
) -> (Comparison, Option<String>) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let home = parent.join(format!(
        "uu-fork-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(why) = std::fs::DirBuilder::new().mode(0o700).create(&home) {
        return (
            Err((
                "fork-clone-unstageable",
                format!(
                    "cannot stage under {}: {why}; check temporary directory and free space",
                    parent.display()
                ),
            )),
            None,
        );
    }
    let result = compare(&home, fields, runner);
    let cleanup = std::fs::remove_dir_all(&home)
        .err()
        .map(|why| format!("fork clone {} could not be removed: {why}", home.display()));
    (result, cleanup)
}
fn compare(home: &Path, fields: &[&str; 3], runner: &dyn CommandRunner) -> Comparison {
    let mut environment = vec!["-i".to_string(), "PATH=/usr/bin:/bin".into()];
    for key in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
        "CLAUDE_CONFIG_DIR",
        "TMPDIR",
    ] {
        environment.push(format!("{key}={}", home.display()));
    }
    environment.extend(
        [
            "GIT_CONFIG_GLOBAL=/dev/null",
            "GIT_CONFIG_SYSTEM=/dev/null",
            "GIT_CONFIG_COUNT=0",
            "GIT_CONFIG_PARAMETERS=",
            "GIT_TERMINAL_PROMPT=0",
            "/usr/bin/git",
        ]
        .map(str::to_string),
    );
    let run = |args: &[&str]| {
        let mut command = environment.iter().map(String::as_str).collect::<Vec<_>>();
        command.extend_from_slice(args);
        runner.run_with_deadline("/usr/bin/env", &command, Duration::from_secs(300))
    };
    let repo = home.join("repo");
    let path = repo.to_string_lossy();
    if let Err(why) = run(&["clone", "--quiet", "--depth", "1", "--", fields[0], &path]) {
        let state = if why.contains("deadline") || why.contains("run's budget") {
            "fork-clone-timeout"
        } else {
            "fork-upstream-unreachable"
        };
        return Err((
            state,
            format!(
                "{}: {why}; not compared, check the recorded URL and network",
                fields[0]
            ),
        ));
    }
    if let Err(why) = run(&[
        "-C",
        &path,
        "rev-parse",
        "--verify",
        "--quiet",
        "HEAD^{commit}",
    ]) {
        return Err((
            "fork-upstream-headless",
            format!(
                "{}: {why}; check the upstream default branch, leave skillPath and lastComparedTreeHash alone",
                fields[0]
            ),
        ));
    }
    let revision = if fields[1] == "." {
        "HEAD^{tree}".into()
    } else {
        format!("HEAD:{}", fields[1])
    };
    run(&["-C", &path, "rev-parse", "--verify", "--quiet", &revision])
        .map(|hash| hash.trim().to_string())
        .map_err(|why| ("fork-path-missing", format!("{}: skillPath {} cannot resolve: {why}; re-point skillPath and leave lastComparedTreeHash alone", fields[0], fields[1])))
}
