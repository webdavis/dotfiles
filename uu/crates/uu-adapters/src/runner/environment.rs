//! A child spawned in an environment the lane chose, and the `PATH` the
//! adapter composes for it.

use super::SystemRunner;
use crate::lanes::{CommandRunner, Environment, Ran};
use crate::watchdog::{Ended, Finished, Spawned, bounded_spawn_in};
use std::ffi::{OsStr, OsString};
use std::process::Stdio;
use std::time::Duration;

pub(super) fn run(
    runner: &SystemRunner,
    program: &str,
    args: &[&str],
    env: &Environment,
    most: Option<Duration>,
) -> Result<String, String> {
    // A step's own bound is held by a second runner for that step alone, the
    // way `bounds::run_step` does it, so every overrun sentence is composed
    // in one place and the lane's clock still charges for the step.
    if let Some(most) = most {
        let bound = most.min(runner.remaining());
        return SystemRunner::for_lane(&format!("{} step {program}", runner.lane), bound, bound)
            .run_in(program, args, env, None);
    }
    runner.clean_output(spawn(runner, program, args, env)?)
}

pub(super) fn run_reporting(
    runner: &SystemRunner,
    program: &str,
    args: &[&str],
    env: &Environment,
) -> Result<Ran, String> {
    Ok(runner.reported(spawn(runner, program, args, env)?))
}

fn spawn(
    runner: &SystemRunner,
    program: &str,
    args: &[&str],
    env: &Environment,
) -> Result<Finished, String> {
    let budget = runner.remaining();
    if budget.is_zero() {
        return Err(runner.overrun(&Ended::Stopped, b""));
    }
    match bounded_spawn_in(program, args, Stdio::null(), budget, env) {
        Spawned::Ran(finished) => Ok(finished),
        Spawned::NotRunnable(why) => Err(why),
        Spawned::SpawnStuck => Err(super::overrun::spawn_stuck(
            &runner.lane,
            runner.budget,
            runner.declared,
            program,
            crate::interruption().is_some(),
        )),
    }
}

/// `prefix` first, then every entry the inherited `PATH` held.
///
/// AN ABSENT OR EMPTY INHERITED VALUE CONTRIBUTES NOTHING rather than an
/// empty entry: an empty `PATH` element is the working directory, so every
/// helper the child could not find in `prefix` would otherwise be answered
/// from wherever uu was started. An empty element the inherited value itself
/// carries is preserved, because it belongs to whoever set that value.
pub(crate) fn prefixed_path(prefix: &str, inherited: Option<&OsStr>) -> OsString {
    let Some(rest) = inherited.filter(|value| !value.is_empty()) else {
        return prefix.into();
    };
    let entries = std::iter::once(std::path::PathBuf::from(prefix))
        .chain(std::env::split_paths(rest))
        .collect::<Vec<_>>();
    // `join_paths` refuses an entry holding the separator, which splitting the
    // inherited value cannot produce; the prefix alone is the fallback.
    std::env::join_paths(entries).unwrap_or_else(|_| prefix.into())
}

#[cfg(test)]
mod tests {
    use super::prefixed_path;
    use std::ffi::OsStr;

    #[test]
    fn the_prefix_comes_first_and_the_inherited_entries_follow_in_order() {
        assert_eq!(
            prefixed_path("/fnm/bin", Some(OsStr::new("/usr/bin:/bin"))),
            OsStr::new("/fnm/bin:/usr/bin:/bin")
        );
    }

    #[test]
    fn an_absent_or_empty_inherited_path_leaves_the_child_no_empty_entry() {
        // An empty PATH element is the WORKING DIRECTORY, so a child that
        // cannot find a helper in the prefix would run whatever sits in the
        // directory uu was started in.
        assert_eq!(prefixed_path("/fnm/bin", None), OsStr::new("/fnm/bin"));
        assert_eq!(
            prefixed_path("/fnm/bin", Some(OsStr::new(""))),
            OsStr::new("/fnm/bin")
        );
    }

    #[test]
    fn an_empty_entry_the_inherited_value_carries_is_preserved() {
        assert_eq!(
            prefixed_path("/fnm/bin", Some(OsStr::new("/usr/bin:"))),
            OsStr::new("/fnm/bin:/usr/bin:")
        );
    }
}

#[cfg(test)]
mod children {
    use super::SystemRunner;
    use crate::lanes::{CommandRunner, Environment, Verdict};
    use std::time::Duration;

    /// A runner whose budget no honest test child comes near.
    fn runner() -> SystemRunner {
        SystemRunner::for_lane("test", Duration::from_secs(30), Duration::from_secs(30))
    }

    /// The child prints the PATH it was actually given.
    const PRINT_PATH: [&str; 2] = ["-c", r#"printf %s "$PATH""#];

    #[test]
    fn a_prefixed_path_reaches_the_child_ahead_of_the_inherited_value() {
        // The npm lane's whole point: npm found on another node's PATH
        // installs into that node's prefix.
        let inherited = std::env::var("PATH").expect("the test process has a PATH");
        let composed = runner()
            .run_in(
                "/bin/sh",
                &PRINT_PATH,
                &Environment::inheriting().prepending_path("/fnm/bin"),
                None,
            )
            .expect("the child runs");
        assert_eq!(composed, format!("/fnm/bin:{inherited}"));
    }

    #[test]
    fn a_variable_set_over_the_inherited_environment_keeps_the_rest_of_it() {
        let named = runner()
            .run_in(
                "/bin/sh",
                &["-c", r#"printf '%s %s' "$UU_FIXTURE" "${PATH:+set}""#],
                &Environment::inheriting().with("UU_FIXTURE", "owned".into()),
                None,
            )
            .expect("the child runs");
        assert_eq!(named, "owned set");
    }

    #[test]
    fn an_isolated_environment_hands_the_child_nothing_else() {
        let named = runner()
            .run_in(
                "/bin/sh",
                &PRINT_PATH,
                &Environment::only(&std::collections::BTreeMap::from([(
                    "PATH".to_string(),
                    "/usr/bin:/bin".to_string(),
                )])),
                None,
            )
            .expect("the child runs");
        assert_eq!(named, "/usr/bin:/bin");
    }

    #[test]
    fn a_reporting_child_keeps_both_pipes_and_its_verdict() {
        let ran = runner()
            .run_reporting_in(
                "/bin/sh",
                &[
                    "-c",
                    r#"printf 'out\n'; printf '%s\n' "$UU_FIXTURE" >&2; exit 4"#,
                ],
                &Environment::inheriting().with("UU_FIXTURE", "said on stderr".into()),
            )
            .expect("the child runs");
        assert_eq!(ran.stdout, "out\n");
        assert_eq!(ran.stderr, "said on stderr\n");
        assert!(
            matches!(ran.verdict, Verdict::Failed(ref why) if why.contains("exit 4")),
            "{:?}",
            ran.verdict
        );
    }
}
