//! What one step of the lane contributes to the report.
//!
//! CONTINUE ON FAILURE, like every lane: a step that failed is counted, named
//! and left behind, because the next attempt is a week away and a run that
//! aborts at its first problem throws away every subject it had not reached.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::lanes::{CommandRunner, Environment, Verdict};
use uu_domain::LaneReport;

/// What the record says about one step. THE REASON, not only the status:
/// `exit 1` alone sends the operator to a log the week may have rotated away,
/// and the command already said why on stderr.
///
/// A step's own STDOUT is not kept. `brew upgrade` prints a build log, and one
/// lane's output must not crowd every other lane out of the record; the change
/// sections are what say what the week did.
pub fn note(report: &mut LaneReport, label: &str, outcome: Result<String, String>) {
    match outcome {
        Ok(_) => report.noted(format!("{label}: ok")),
        Err(why) => report.failed(format!("{label}: {why}")),
    }
}

/// One command, bounded only by what is left of the lane's own deadline.
pub fn step(
    report: &mut LaneReport,
    runner: &dyn CommandRunner,
    label: &str,
    program: &str,
    args: &[&str],
) {
    note(report, label, runner.run(program, args));
}

/// `brew upgrade`, read for the taps Homebrew skipped. Homebrew exits 0 when
/// tap trust keeps it from loading a tap, so the exit alone records `ok` for a
/// run in which nothing from that tap was upgraded.
pub fn upgrade_step(report: &mut LaneReport, runner: &dyn CommandRunner, program: &str) {
    let label = "brew upgrade";
    match runner.run_reporting_in(program, &["upgrade"], &Environment::inheriting()) {
        Err(why) => report.failed(format!("{label}: {why}")),
        Ok(ran) => match ran.verdict {
            Verdict::Clean => {
                let skipped = untrusted_taps(&ran.stderr);
                if skipped.is_empty() {
                    report.noted(format!("{label}: ok"));
                } else {
                    let word = if skipped.len() == 1 { "tap" } else { "taps" };
                    report.failed(format!(
                        "{label}: Homebrew skipped {} untrusted {word}, so nothing from them was \
                         upgraded: {}",
                        skipped.len(),
                        skipped.into_iter().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            Verdict::Failed(why) | Verdict::Deferred(why) | Verdict::Pending(why) => {
                report.failed(format!("{label}: {why}"))
            }
        },
    }
}

/// `brew tap-info --installed --json`, read for a tap left untrusted. A
/// direct read of Homebrew's own trust store, independent of whether
/// anything was outdated this week: `brew upgrade` only reaches the check
/// that names an untrusted tap after `return if outdated.blank?`, so a week
/// with nothing else to upgrade records `brew upgrade: ok` even while a tap
/// sits untrusted.
pub fn trust_step(report: &mut LaneReport, runner: &dyn CommandRunner, program: &str) {
    let label = "brew tap-info";
    match runner.run(program, &["tap-info", "--installed", "--json"]) {
        Err(why) => report.failed(format!("{label}: {why}")),
        Ok(stdout) => {
            let untrusted = untrusted_installed_taps(&stdout);
            if untrusted.is_empty() {
                report.noted(format!("{label}: ok"));
            } else {
                let word = if untrusted.len() == 1 { "tap" } else { "taps" };
                report.failed(format!(
                    "{label}: {} installed {word} not trusted: {}",
                    untrusted.len(),
                    untrusted.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }
}

/// Every tap named in `brew tap-info --installed --json` whose own `trusted`
/// field is false. Output this cannot parse (a brew version whose JSON shape
/// moved) is read as nothing untrusted rather than failing the step, the same
/// choice `untrusted_taps` below makes for its own text format.
fn untrusted_installed_taps(stdout: &str) -> BTreeSet<String> {
    let Ok(serde_json::Value::Array(taps)) = serde_json::from_str(stdout) else {
        return BTreeSet::new();
    };
    taps.into_iter()
        .filter_map(|tap| {
            let name = tap.get("name")?.as_str()?.to_string();
            let trusted = tap.get("trusted")?.as_bool()?;
            (!trusted).then_some(name)
        })
        .collect()
}

/// Every tap Homebrew's stderr says it skipped as untrusted, whether named in
/// a `Skipping <tap> because it is not trusted` line or listed under `The
/// following taps are not trusted:`.
fn untrusted_taps(stderr: &str) -> BTreeSet<String> {
    let mut taps = BTreeSet::new();
    let mut listing = false;
    for line in stderr.lines() {
        if listing {
            match line.strip_prefix("  ").map(str::trim) {
                Some(tap) if !tap.is_empty() => {
                    taps.insert(tap.to_string());
                    continue;
                }
                _ => listing = false,
            }
        }
        if line.ends_with("The following taps are not trusted:") {
            listing = true;
        } else if let Some((_, rest)) = line.split_once("Skipping ")
            && let Some((tap, _)) = rest.split_once(" because it is not trusted")
        {
            taps.insert(tap.to_string());
        }
    }
    taps
}

/// One command under a bound of its own, for a subject that WEDGES rather
/// than fails. The App Store hangs indefinitely on a broken session, and the
/// lane deadline covers the whole lane, so an unbounded mas step is a week in
/// which nothing after it ran rather than one failed step.
pub fn bounded_step(
    report: &mut LaneReport,
    runner: &dyn CommandRunner,
    label: &str,
    program: &str,
    args: &[&str],
    most: Duration,
) {
    note(report, label, runner.run_with_deadline(program, args, most));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::stubs::ScriptedRunner;

    fn report() -> LaneReport {
        LaneReport::new("brew")
    }

    #[test]
    fn a_step_that_failed_carries_the_reason_the_command_gave_not_only_its_label() {
        let mut report = report();
        note(
            &mut report,
            "brew upgrade",
            Err("exit 1: no such tap".into()),
        );
        assert_eq!(report.failures(), 1);
        assert_eq!(report.lines, vec!["brew upgrade: exit 1: no such tap"]);
    }

    #[test]
    fn a_step_that_worked_is_recorded_without_the_build_log_it_printed() {
        let mut report = report();
        note(&mut report, "brew upgrade", Ok("pages of output".into()));
        assert_eq!(report.failures(), 0);
        assert_eq!(report.lines, vec!["brew upgrade: ok"]);
    }

    #[test]
    fn an_ordinary_step_runs_the_program_and_its_arguments_under_no_bound_of_its_own() {
        let runner = ScriptedRunner::new(&[]);
        let mut report = report();
        step(&mut report, &runner, "brew update", "/b/brew", &["update"]);
        assert_eq!(runner.calls(), vec![vec!["/b/brew", "update"]]);
        assert!(runner.deadlines().is_empty(), "{:?}", runner.deadlines());
    }

    #[test]
    fn an_upgrade_step_with_one_untrusted_tap_names_it_in_the_singular() {
        let runner = ScriptedRunner::new(&[]).saying_on_stderr(
            &["/b/brew", "upgrade"],
            "Warning: The following taps are not trusted:\n  rjyo/moshi\n\n",
        );
        let mut report = report();
        upgrade_step(&mut report, &runner, "/b/brew");
        assert_eq!(report.failures(), 1);
        assert_eq!(
            report.lines,
            vec![
                "brew upgrade: Homebrew skipped 1 untrusted tap, so nothing from them was upgraded: rjyo/moshi"
            ]
        );
    }

    #[test]
    fn a_trust_step_with_every_installed_tap_trusted_is_ok() {
        let runner = ScriptedRunner::new(&[]).answering(
            r#"[{"name":"homebrew/core","trusted":true},{"name":"rjyo/moshi","trusted":true}]"#,
        );
        let mut report = report();
        trust_step(&mut report, &runner, "/b/brew");
        assert_eq!(report.failures(), 0, "{report:?}");
        assert_eq!(report.lines, vec!["brew tap-info: ok"]);
    }

    #[test]
    fn a_trust_step_with_one_untrusted_tap_names_it_in_the_singular() {
        let runner = ScriptedRunner::new(&[]).answering(
            r#"[{"name":"rjyo/moshi","trusted":false},{"name":"homebrew/core","trusted":true}]"#,
        );
        let mut report = report();
        trust_step(&mut report, &runner, "/b/brew");
        assert_eq!(report.failures(), 1);
        assert_eq!(
            report.lines,
            vec!["brew tap-info: 1 installed tap not trusted: rjyo/moshi"]
        );
    }

    #[test]
    fn a_trust_step_with_two_untrusted_taps_names_both_sorted() {
        let runner = ScriptedRunner::new(&[]).answering(
            r#"[{"name":"steipete/tap","trusted":false},{"name":"rjyo/moshi","trusted":false}]"#,
        );
        let mut report = report();
        trust_step(&mut report, &runner, "/b/brew");
        assert_eq!(report.failures(), 1);
        assert_eq!(
            report.lines,
            vec!["brew tap-info: 2 installed taps not trusted: rjyo/moshi, steipete/tap"]
        );
    }

    #[test]
    fn a_trust_step_reading_output_it_cannot_parse_is_ok_rather_than_failing_the_step() {
        let runner = ScriptedRunner::new(&[]).answering("not json");
        let mut report = report();
        trust_step(&mut report, &runner, "/b/brew");
        assert_eq!(report.failures(), 0, "{report:?}");
        assert_eq!(report.lines, vec!["brew tap-info: ok"]);
    }

    #[test]
    fn a_bounded_step_is_given_its_own_deadline_rather_than_the_lanes() {
        let runner = ScriptedRunner::new(&[]);
        let mut report = report();
        bounded_step(
            &mut report,
            &runner,
            "mas upgrade",
            "/b/mas",
            &["upgrade"],
            Duration::from_secs(180),
        );
        assert_eq!(
            runner.deadlines(),
            vec![(
                vec!["/b/mas".to_string(), "upgrade".to_string()],
                Duration::from_secs(180)
            )]
        );
    }
}
