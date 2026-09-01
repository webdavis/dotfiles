//! The lane registry, and the first lane.
//!
//! CONTINUE ON FAILURE, at both levels. A plugin that will not reinstall does
//! not stop the next plugin, and a lane that failed does not stop the next
//! lane: the run completes, the record says what failed, and the exit status
//! stays 0. The scheduler's retry is a whole week away, so a run that aborts
//! at its first problem throws away every subject it had not reached yet.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install, which re-pins at the source's tip. A failed install RETRIES
//! ONCE and a plugin that still failed is named loudly, so the record for the
//! week says exactly what is missing. The running server keeps its loaded
//! plugins until restart, so a failure costs the next restart rather than the
//! current session.

use crate::config::{Config, HerdrLane, LANE_NAMES};

/// What one lane did: how many things went wrong, and the lines the record
/// carries about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneReport {
    pub name: String,
    pub failures: usize,
    pub lines: Vec<String>,
}

/// The spawn seam. `Ok` carries the command's stdout, `Err` why it did not
/// succeed, already fit to print.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;
}

/// The lanes this config turns on, in the roster's own order.
///
/// ORDER IS THE ROSTER'S, never the config file's. A TOML table order is
/// whatever the operator happened to type, and a run whose sequence changes
/// when a block moves is a run nobody can reason about.
pub fn enabled_lanes(config: &Config) -> Vec<&'static str> {
    LANE_NAMES
        .iter()
        .copied()
        .filter(|name| is_enabled(name, config))
        .collect()
}

/// Whether one named lane has a block in this config.
fn is_enabled(name: &str, config: &Config) -> bool {
    match name {
        "herdr" => config.lanes.herdr.is_some(),
        _ => false,
    }
}

/// Run one named lane, or `None` when this config leaves it off.
pub fn run_lane(name: &str, config: &Config, runner: &dyn CommandRunner) -> Option<LaneReport> {
    match name {
        "herdr" => config
            .lanes
            .herdr
            .as_ref()
            .map(|lane| run_herdr(lane, runner)),
        _ => None,
    }
}

/// The herdr lane: the binary refreshes itself, then every plugin in the
/// roster is reinstalled at its source's tip.
pub fn run_herdr(lane: &HerdrLane, runner: &dyn CommandRunner) -> LaneReport {
    let mut report = LaneReport {
        name: "herdr".to_string(),
        failures: 0,
        lines: Vec::new(),
    };

    match runner.run(&lane.binary, &["update"]) {
        Ok(_) => {
            // The version is a COURTESY in the record and never a verdict: a
            // build that will not print its own version still updated.
            let version = runner
                .run(&lane.binary, &["--version"])
                .ok()
                .and_then(|out| out.lines().next().map(str::to_string))
                .filter(|line| !line.is_empty());
            report.lines.push(match version {
                Some(version) => format!("herdr self-update: ok ({version})"),
                None => "herdr self-update: ok".to_string(),
            });
        }
        Err(why) => {
            report.failures += 1;
            report.lines.push(format!(
                "herdr self-update FAILED ({why}); plugins still refresh below"
            ));
        }
    }

    for plugin in &lane.plugins {
        let id = plugin.id.as_str();
        // AN INSTALL OVER A FAILED UNINSTALL IS NOT ATTEMPTED. herdr pins a
        // plugin at install, so installing on top of a copy that would not
        // come off is how one plugin becomes two.
        if runner
            .run(&lane.binary, &["plugin", "uninstall", id])
            .is_err()
        {
            report.failures += 1;
            report.lines.push(format!(
                "plugin {id}: uninstall failed; leaving the installed copy alone"
            ));
            continue;
        }
        let install = || runner.run(&lane.binary, &["plugin", "install", &plugin.repo, "--yes"]);
        if install().is_ok() || install().is_ok() {
            report.lines.push(format!("plugin {id}: refreshed"));
        } else {
            report.failures += 1;
            report.lines.push(format!(
                "plugin {id}: REINSTALL FAILED twice; it is now MISSING until the next apply or run"
            ));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Plugin, parse_config};
    use std::cell::RefCell;

    /// A runner that answers from a script and records every call. The script
    /// is keyed on the whole argument vector, so a test says exactly which
    /// invocation fails without depending on call order.
    struct ScriptedRunner {
        failing: Vec<Vec<String>>,
        stdout: String,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptedRunner {
        fn new(failing: &[&[&str]]) -> Self {
            ScriptedRunner {
                failing: failing
                    .iter()
                    .map(|call| call.iter().map(|word| word.to_string()).collect())
                    .collect(),
                stdout: String::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn answering(mut self, stdout: &str) -> Self {
            self.stdout = stdout.to_string();
            self
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }
    }

    impl CommandRunner for ScriptedRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().map(|word| word.to_string()));
            self.calls.borrow_mut().push(call.clone());
            // The Nth repeat of a scripted failure is still a failure: the
            // retry has to be able to fail too.
            if self.failing.contains(&call) {
                return Err("exit 1".to_string());
            }
            Ok(self.stdout.clone())
        }
    }

    fn lane(plugins: &[(&str, &str)]) -> HerdrLane {
        HerdrLane {
            binary: "herdr".to_string(),
            plugins: plugins
                .iter()
                .map(|(id, repo)| Plugin {
                    id: id.to_string(),
                    repo: repo.to_string(),
                })
                .collect(),
        }
    }

    // --- the registry ---------------------------------------------------------

    #[test]
    fn a_config_with_no_lane_block_enables_nothing() {
        let config = parse_config("").unwrap();
        assert!(enabled_lanes(&config).is_empty());
        assert_eq!(
            run_lane("herdr", &config, &ScriptedRunner::new(&[])),
            None,
            "a lane with no block must not run just because it was named"
        );
    }

    #[test]
    fn a_lane_block_with_nothing_in_it_turns_the_lane_on() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(enabled_lanes(&config), vec!["herdr"]);
        assert!(run_lane("herdr", &config, &ScriptedRunner::new(&[])).is_some());
    }

    #[test]
    fn a_lane_this_build_does_not_have_runs_nothing() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(run_lane("brew", &config, &ScriptedRunner::new(&[])), None);
    }

    #[test]
    fn every_lane_the_roster_names_can_be_selected_and_run() {
        // The roster is what `uu run <lane>` validates against, so a name in it
        // that dispatches to nothing would accept a lane it never runs.
        for name in LANE_NAMES {
            let config = parse_config(&format!("[lanes.{name}]\n")).unwrap();
            assert!(
                run_lane(name, &config, &ScriptedRunner::new(&[])).is_some(),
                "the roster names `{name}` but nothing dispatches it"
            );
        }
    }

    // --- the herdr lane -------------------------------------------------------

    #[test]
    fn a_clean_run_updates_herdr_then_refreshes_every_plugin_in_roster_order() {
        let runner = ScriptedRunner::new(&[]);
        let report = run_herdr(&lane(&[("a", "o/a"), ("b", "o/b")]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls(),
            vec![
                vec!["herdr", "update"],
                vec!["herdr", "--version"],
                vec!["herdr", "plugin", "uninstall", "a"],
                vec!["herdr", "plugin", "install", "o/a", "--yes"],
                vec!["herdr", "plugin", "uninstall", "b"],
                vec!["herdr", "plugin", "install", "o/b", "--yes"],
            ]
        );
    }

    #[test]
    fn the_configured_binary_is_the_one_that_runs() {
        let runner = ScriptedRunner::new(&[]);
        let mut configured = lane(&[]);
        configured.binary = "/opt/herdr".to_string();
        run_herdr(&configured, &runner);
        assert_eq!(runner.calls()[0][0], "/opt/herdr");
    }

    #[test]
    fn a_failed_self_update_is_counted_and_the_plugins_still_refresh() {
        let runner = ScriptedRunner::new(&[&["herdr", "update"]]);
        let report = run_herdr(&lane(&[("a", "o/a")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("self-update FAILED")),
            "{:?}",
            report.lines
        );
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin a: refreshed"),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_successful_self_update_reports_the_version_it_landed_on() {
        let runner = ScriptedRunner::new(&[]).answering("herdr 0.42.0\nbuilt from source\n");
        let report = run_herdr(&lane(&[]), &runner);
        assert_eq!(report.lines[0], "herdr self-update: ok (herdr 0.42.0)");
    }

    #[test]
    fn a_version_that_will_not_answer_still_leaves_the_update_reported_as_ok() {
        // The version is a courtesy in the record, never a verdict: counting it
        // as a failure would report a healthy update as a broken one.
        let runner = ScriptedRunner::new(&[&["herdr", "--version"]]);
        let report = run_herdr(&lane(&[]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(report.lines[0], "herdr self-update: ok");
    }

    #[test]
    fn a_plugin_whose_uninstall_fails_is_left_installed_and_never_reinstalled() {
        // Installing over a plugin the uninstall could not remove is how a
        // half-removed plugin becomes two.
        let runner = ScriptedRunner::new(&[&["herdr", "plugin", "uninstall", "a"]]);
        let report = run_herdr(&lane(&[("a", "o/a"), ("b", "o/b")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("plugin a: uninstall failed")),
            "{:?}",
            report.lines
        );
        assert!(
            !runner
                .calls()
                .iter()
                .any(|call| call.contains(&"o/a".to_string())),
            "{:?}",
            runner.calls()
        );
        // And the next plugin is still refreshed.
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin b: refreshed"),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_failed_install_is_retried_exactly_once_and_a_second_attempt_that_works_is_a_refresh() {
        struct FlakyInstall {
            attempts: RefCell<usize>,
        }
        impl CommandRunner for FlakyInstall {
            fn run(&self, _program: &str, args: &[&str]) -> Result<String, String> {
                if args.first() == Some(&"plugin") && args.get(1) == Some(&"install") {
                    let mut attempts = self.attempts.borrow_mut();
                    *attempts += 1;
                    if *attempts == 1 {
                        return Err("exit 1".to_string());
                    }
                }
                Ok(String::new())
            }
        }
        let runner = FlakyInstall {
            attempts: RefCell::new(0),
        };
        let report = run_herdr(&lane(&[("a", "o/a")]), &runner);
        assert_eq!(
            *runner.attempts.borrow(),
            2,
            "one retry, not none and not a loop"
        );
        assert_eq!(report.failures, 0);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin a: refreshed")
        );
    }

    #[test]
    fn a_plugin_that_fails_twice_is_named_loudly_as_missing() {
        let runner = ScriptedRunner::new(&[&["herdr", "plugin", "install", "o/a", "--yes"]]);
        let report = run_herdr(&lane(&[("a", "o/a")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("plugin a: REINSTALL FAILED twice")
                    && line.contains("MISSING")),
            "{:?}",
            report.lines
        );
        assert_eq!(
            runner
                .calls()
                .iter()
                .filter(|call| call.contains(&"install".to_string()))
                .count(),
            2
        );
    }

    #[test]
    fn every_failure_across_the_lane_is_counted_once() {
        let runner = ScriptedRunner::new(&[
            &["herdr", "update"],
            &["herdr", "plugin", "uninstall", "a"],
            &["herdr", "plugin", "install", "o/b", "--yes"],
        ]);
        let report = run_herdr(&lane(&[("a", "o/a"), ("b", "o/b"), ("c", "o/c")]), &runner);
        assert_eq!(report.failures, 3);
        assert_eq!(report.name, "herdr");
    }

    #[test]
    fn a_lane_with_no_plugins_still_updates_the_binary() {
        let runner = ScriptedRunner::new(&[]);
        let report = run_herdr(&lane(&[]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls().first().map(|call| call[1].clone()),
            Some("update".to_string())
        );
    }
}
