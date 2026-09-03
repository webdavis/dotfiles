//! The brew lane: Homebrew formulae, casks and Mac App Store apps, plus the
//! two repairs only this lane is positioned to make.
//!
//! THE ORDER IS THE DESIGN. The osquery converge runs IMMEDIATELY after
//! `brew upgrade`, not at the end of the lane: between a cask upgrade wiping
//! /var/osquery and this repairing it, the root daemon is running without our
//! detection config, and `mas upgrade` plus `brew cleanup` can take minutes.
//! That window is a monitoring gap, so it is kept as short as the ordering
//! allows.

pub mod changes;
pub mod repairs;
pub mod sections;
pub mod steps;
pub mod upgrade_record;

use std::time::Duration;

use crate::config::BrewLane;
use crate::lanes::{CommandRunner, LaneReport};
use crate::record::RunFacts;

use changes::{Listing, parse_brew_versions, parse_mas_list, tuple_row, tuples};
use repairs::{converge_osquery, mas_declarations, refresh_tailscaled};
use sections::change_section;
use steps::{bounded_step, step};

/// How long ONE App Store step may take. The store hangs indefinitely on a
/// wedged session, and the lane's own deadline covers the whole lane, so an
/// unbounded mas step is a week in which nothing after it ran.
const MAS_DEADLINE: Duration = Duration::from_secs(180);

/// What each subject CANNOT tell you, restated on every entry rather than
/// assumed known.
const BREW_CAVEAT: &str = "Versions are what brew list --versions reports; a formula reinstalled \
                           at the same version does not appear here, and a cask Homebrew tracks \
                           only as latest reports that literal string rather than a version.";
const MAS_CAVEAT: &str = "Versions are what mas list reports, keyed by app name.";

pub fn run_brew(
    name: &str,
    lane: &BrewLane,
    facts: &RunFacts,
    runner: &dyn CommandRunner,
) -> LaneReport {
    let mut report = LaneReport::new(name);

    let brew_before = read(
        runner,
        &lane.brew,
        &["list", "--versions"],
        parse_brew_versions,
    );
    let mas_before = read(runner, &lane.mas, &["list"], parse_mas_list);
    // PUBLISHED BEFORE THE FIRST UPGRADE STEP, so the record covers the window
    // it describes. Written only at the end, a watched file rewritten in the
    // first seconds of a run is correlated against the PREVIOUS week.
    let _ = upgrade_record::publish(lane, facts, brew_before.is_ok(), &[]);

    step(&mut report, runner, "brew update", &lane.brew, &["update"]);
    step(
        &mut report,
        runner,
        "brew outdated",
        &lane.brew,
        &["outdated"],
    );
    let mas = lane.mas.as_str();
    bounded_step(
        &mut report,
        runner,
        "mas outdated",
        mas,
        &["outdated"],
        MAS_DEADLINE,
    );
    step(
        &mut report,
        runner,
        "brew upgrade",
        &lane.brew,
        &["upgrade"],
    );
    refresh_tailscaled(&mut report, runner, lane);
    converge_osquery(&mut report, runner, lane);
    bounded_step(
        &mut report,
        runner,
        "mas upgrade",
        mas,
        &["upgrade"],
        MAS_DEADLINE,
    );
    mas_declarations(&mut report, runner, lane, MAS_DEADLINE);
    step(
        &mut report,
        runner,
        "brew cleanup",
        &lane.brew,
        &["cleanup"],
    );

    let brew_after = read(
        runner,
        &lane.brew,
        &["list", "--versions"],
        parse_brew_versions,
    );
    let mas_after = read(runner, &lane.mas, &["list"], parse_mas_list);

    // BREW ONLY in the persisted record: App Store apps install into
    // /Applications, which no known-good manifest covers and no file-integrity
    // watch reads, so a mas transition could never explain one of those pages.
    let rows = match (&brew_before, &brew_after) {
        (Ok(before), Ok(after)) => tuples(before, after).iter().map(tuple_row).collect(),
        _ => Vec::new(),
    };
    if let Some(why) = upgrade_record::publish(lane, facts, brew_after.is_ok(), &rows) {
        report.noted(format!(
            "upgrade record: NOT written, {why}; the file-integrity page will report no recorded \
             upgrade for this run"
        ));
    }

    report.noted(change_section(
        &brew_before,
        &brew_after,
        "formulae and casks",
        BREW_CAVEAT,
        "brew list --versions",
    ));
    report.noted(change_section(
        &mas_before,
        &mas_after,
        "App Store apps",
        MAS_CAVEAT,
        "mas list",
    ));
    report
}

/// One reading of what is installed. A reading that FAILED is not a failed
/// step: the upgrade is the work, and the record says the comparison could not
/// be made rather than reporting a quiet week on a subject nothing could ask
/// about.
fn read(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[&str],
    parse: fn(&str) -> Listing,
) -> Result<Listing, String> {
    runner.run(program, args).map(|stdout| parse(&stdout))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::repairs::tests::lane;
    use super::*;
    use crate::lanes::tests::ScriptedRunner;
    use crate::record::Marker;

    const STUB_MARKER: Marker = Marker::NeverRecorded;

    pub(crate) fn facts() -> RunFacts<'static> {
        RunFacts {
            host: "test-host",
            started_epoch: 1_760_000_000,
            started_iso: "2025-10-09T07:33:20Z",
            marker: &STUB_MARKER,
        }
    }

    fn said(report: &LaneReport, wanted: &str) -> bool {
        report.lines.iter().any(|line| line.contains(wanted))
    }

    #[test]
    fn a_clean_run_reads_upgrades_and_repairs_in_the_one_order_that_closes_the_gaps() {
        // The converge sits immediately after `brew upgrade` and before the
        // two steps that take minutes: a cask upgrade wipes /var/osquery, and
        // every second between the wipe and the repair is a monitoring gap.
        let runner = ScriptedRunner::new(&[]);
        let report = run_brew("brew", &lane(), &facts(), &runner);
        assert_eq!(report.failures, 0, "{report:?}");
        assert_eq!(
            runner.calls(),
            vec![
                vec!["/b/brew", "list", "--versions"],
                vec!["/b/mas", "list"],
                vec!["/b/brew", "update"],
                vec!["/b/brew", "outdated"],
                vec!["/b/mas", "outdated"],
                vec!["/b/brew", "upgrade"],
                vec!["/usr/bin/cmp", "-s", "/bin/sh", "/usr/local/bin/tailscaled"],
                vec!["/b/converge"],
                vec!["/b/mas", "upgrade"],
                vec!["/b/brew", "bundle", "--no-upgrade", "--file=/etc/hosts"],
                vec!["/b/brew", "cleanup"],
                vec!["/b/brew", "list", "--versions"],
                vec!["/b/mas", "list"],
            ]
        );
    }

    #[test]
    fn every_app_store_step_runs_under_its_own_bound_and_no_other_step_does() {
        // A wedged store used to cost one Monday job. Under uu it would hold
        // the lane's whole deadline and every step after it.
        let runner = ScriptedRunner::new(&[]);
        run_brew("brew", &lane(), &facts(), &runner);
        let bounded: Vec<Vec<String>> = runner
            .deadlines()
            .into_iter()
            .map(|(call, most)| {
                assert_eq!(most, MAS_DEADLINE, "{call:?}");
                call
            })
            .collect();
        assert_eq!(
            bounded,
            vec![
                vec!["/b/mas".to_string(), "outdated".to_string()],
                vec!["/b/mas".to_string(), "upgrade".to_string()],
                vec![
                    "/b/brew".to_string(),
                    "bundle".to_string(),
                    "--no-upgrade".to_string(),
                    "--file=/etc/hosts".to_string(),
                ],
            ]
        );
    }

    #[test]
    fn a_failed_step_is_counted_and_named_and_every_later_step_still_runs() {
        // The next attempt is a week away, so a run that aborts at its first
        // problem throws away every subject it had not reached.
        let runner = ScriptedRunner::new(&[&["/b/brew", "upgrade"]]);
        let report = run_brew("brew", &lane(), &facts(), &runner);
        assert_eq!(report.failures, 1);
        assert!(said(&report, "brew upgrade: exit 1"), "{report:?}");
        assert!(
            runner
                .calls()
                .contains(&vec!["/b/brew".to_string(), "cleanup".to_string()]),
            "{:?}",
            runner.calls()
        );
    }

    #[test]
    fn each_subject_is_read_with_its_own_command_and_reported_under_its_own_label() {
        // One listing text answers both readings here, and only the brew
        // parser finds an entry in it: a lane that read mas with the brew
        // parser, or labelled either section with the other's name, changes
        // both counts below.
        let runner = ScriptedRunner::new(&[]).answering("jq 1.7.1\n");
        let report = run_brew("brew", &lane(), &facts(), &runner);
        assert!(
            said(
                &report,
                "formulae and casks: 0 of 1 tracked entries changed"
            ),
            "{report:?}"
        );
        assert!(
            said(&report, "App Store apps: 0 of 0 tracked entries changed"),
            "{report:?}"
        );
    }

    #[test]
    fn a_listing_that_could_not_be_read_says_so_instead_of_reading_as_a_quiet_week() {
        let runner = ScriptedRunner::new(&[&["/b/mas", "list"]]);
        let report = run_brew("brew", &lane(), &facts(), &runner);
        assert!(said(&report, "App Store apps: NOT COMPARED"), "{report:?}");
        assert!(said(&report, "mas list"), "{report:?}");
        // The reading is not a STEP: the upgrade is the work, and a failed
        // reading costs the comparison rather than the run.
        assert_eq!(report.failures, 0, "{report:?}");
    }
}
