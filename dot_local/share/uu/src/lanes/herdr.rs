//! The herdr lane: the binary refreshes itself, then every plugin in the
//! roster is reinstalled at its source's tip.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install, which re-pins at the source's tip. A failed install RETRIES
//! ONCE and a plugin that still failed is named loudly, so the record for the
//! week says exactly what is missing. The running server keeps its loaded
//! plugins until restart, so a failure costs the next restart rather than the
//! current session.
//!
//! THE RUN EVENT IS UNUSED HERE: this lane predates it and drives herdr by
//! argv alone.

use crate::config::HerdrLane;
use crate::lanes::{CommandRunner, LaneAdapter, LaneReport};
use crate::record::RunFacts;

impl LaneAdapter for HerdrLane {
    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);

        match runner.run(&self.binary, &["update"]) {
            Ok(_) => {
                // The version is a COURTESY in the record and never a verdict:
                // a build that will not print its own version still updated.
                let version = runner
                    .run(&self.binary, &["--version"])
                    .ok()
                    .and_then(|out| out.lines().next().map(str::to_string))
                    .filter(|line| !line.is_empty());
                report.noted(match version {
                    Some(version) => format!("herdr self-update: ok ({version})"),
                    None => "herdr self-update: ok".to_string(),
                });
            }
            Err(why) => report.failed(format!(
                "herdr self-update FAILED ({why}); plugins still refresh below"
            )),
        }

        for plugin in &self.plugins {
            let id = plugin.id.as_str();
            // AN INSTALL OVER A FAILED UNINSTALL IS NOT ATTEMPTED. herdr pins
            // a plugin at install, so installing on top of a copy that would
            // not come off is how one plugin becomes two.
            if let Err(why) = runner.run(&self.binary, &["plugin", "uninstall", id]) {
                report.failed(format!(
                    "plugin {id}: uninstall failed ({why}); leaving the installed copy alone"
                ));
                continue;
            }
            let install =
                || runner.run(&self.binary, &["plugin", "install", &plugin.repo, "--yes"]);
            // The retry is the SECOND call and there is no third: `or_else`
            // runs it only on a failure, and the reason kept is the one the
            // last attempt gave.
            match install().or_else(|_| install()) {
                Ok(_) => report.noted(format!("plugin {id}: refreshed")),
                Err(why) => report.failed(format!(
                    "plugin {id}: REINSTALL FAILED twice ({why}); it is now MISSING until the \
                     next apply or run"
                )),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Plugin;
    use crate::lanes::stubs::{ScriptedRunner, stub_facts};
    use crate::lanes::{Ran, Verdict};
    use std::cell::RefCell;
    use std::time::Duration;

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

    // --- the herdr lane -------------------------------------------------------

    #[test]
    fn a_clean_run_updates_herdr_then_refreshes_every_plugin_in_roster_order() {
        let runner = ScriptedRunner::new(&[]);
        let report = lane(&[("a", "o/a"), ("b", "o/b")]).run("herdr", &stub_facts(), &runner);
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
        configured.run("herdr", &stub_facts(), &runner);
        assert_eq!(runner.calls()[0][0], "/opt/herdr");
    }

    #[test]
    fn a_failed_self_update_is_counted_and_the_plugins_still_refresh() {
        let runner = ScriptedRunner::new(&[&["herdr", "update"]]);
        let report = lane(&[("a", "o/a")]).run("herdr", &stub_facts(), &runner);
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
        let report = lane(&[]).run("herdr", &stub_facts(), &runner);
        assert_eq!(report.lines[0], "herdr self-update: ok (herdr 0.42.0)");
    }

    #[test]
    fn a_version_that_will_not_answer_still_leaves_the_update_reported_as_ok() {
        // The version is a courtesy in the record, never a verdict: counting it
        // as a failure would report a healthy update as a broken one.
        let runner = ScriptedRunner::new(&[&["herdr", "--version"]]);
        let report = lane(&[]).run("herdr", &stub_facts(), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(report.lines[0], "herdr self-update: ok");
    }

    #[test]
    fn a_plugin_whose_uninstall_fails_is_left_installed_and_never_reinstalled() {
        // Installing over a plugin the uninstall could not remove is how a
        // half-removed plugin becomes two.
        let runner = ScriptedRunner::new(&[&["herdr", "plugin", "uninstall", "a"]]);
        let report = lane(&[("a", "o/a"), ("b", "o/b")]).run("herdr", &stub_facts(), &runner);
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

            fn run_with_deadline(
                &self,
                program: &str,
                args: &[&str],
                _most: Duration,
            ) -> Result<String, String> {
                self.run(program, args)
            }

            fn run_with_input(
                &self,
                program: &str,
                args: &[&str],
                _input: &str,
            ) -> Result<Ran, String> {
                match self.run(program, args) {
                    Ok(stdout) => Ok(Ran {
                        stdout,
                        verdict: Verdict::Clean,
                    }),
                    Err(failure) => Ok(Ran {
                        stdout: String::new(),
                        verdict: Verdict::Failed(failure),
                    }),
                }
            }
        }
        let runner = FlakyInstall {
            attempts: RefCell::new(0),
        };
        let report = lane(&[("a", "o/a")]).run("herdr", &stub_facts(), &runner);
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
        let report = lane(&[("a", "o/a")]).run("herdr", &stub_facts(), &runner);
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
    fn a_failure_followed_by_a_success_is_summarized_by_the_failure() {
        // The lane CONTINUES after a failure, so its last line is routinely a
        // later success. An alert reading `1 failure(s); plugin a: refreshed`
        // is a card that names nothing to fix and reads like a lane that
        // worked.
        let runner = ScriptedRunner::new(&[&["herdr", "update"]]);
        let report = lane(&[("a", "o/a")]).run("herdr", &stub_facts(), &runner);
        let summary = crate::alert::alert_summary(&report);
        assert!(summary.contains("self-update FAILED"), "{summary}");
        assert!(!summary.contains("refreshed"), "{summary}");
    }

    #[test]
    fn a_failed_step_names_the_reason_the_command_gave() {
        // `exit 1` alone sends the operator to a log a weekly job may have
        // rotated away. The command already said why on stderr, and the
        // record and the alert are where that has to end up.
        let runner = ScriptedRunner::new(&[
            &["herdr", "plugin", "uninstall", "a"],
            &["herdr", "plugin", "install", "o/b", "--yes"],
        ]);
        let report = lane(&[("a", "o/a"), ("b", "o/b")]).run("herdr", &stub_facts(), &runner);
        for line in report
            .lines
            .iter()
            .filter(|line| line.contains("FAILED") || line.contains("failed"))
        {
            assert!(line.contains("exit 1"), "{line}");
        }
        assert_eq!(report.failures, 2);
    }

    #[test]
    fn every_failure_across_the_lane_is_counted_once() {
        let runner = ScriptedRunner::new(&[
            &["herdr", "update"],
            &["herdr", "plugin", "uninstall", "a"],
            &["herdr", "plugin", "install", "o/b", "--yes"],
        ]);
        let report =
            lane(&[("a", "o/a"), ("b", "o/b"), ("c", "o/c")]).run("herdr", &stub_facts(), &runner);
        assert_eq!(report.failures, 3);
        assert_eq!(report.name, "herdr");
    }

    #[test]
    fn a_lane_with_no_plugins_still_updates_the_binary() {
        let runner = ScriptedRunner::new(&[]);
        let report = lane(&[]).run("herdr", &stub_facts(), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls().first().map(|call| call[1].clone()),
            Some("update".to_string())
        );
    }
}
