use super::*;
use crate::lanes::stubs::ScriptedRunner;

/// `/bin/sh` stands in for an installed tailscaled and `/etc/hosts` for a
/// published manifest: both exist on every host this runs on, and neither
/// is ever executed here, because the runner is a double.
pub(crate) fn lane() -> BrewLane {
    BrewLane {
        brew: "/b/brew".to_string(),
        mas: "/b/mas".to_string(),
        tailscaled: "/bin/sh".to_string(),
        osquery_converge: vec!["/b/converge".to_string()],
        mas_manifest: "/etc/hosts".to_string(),
        upgrade_record: String::new(),
    }
}

fn report() -> LaneReport {
    LaneReport::new("brew")
}

fn said(report: &LaneReport, wanted: &str) -> bool {
    report.lines.iter().any(|line| line.contains(wanted))
}

#[test]
fn a_tailscale_that_is_not_installed_leaves_the_system_daemon_alone() {
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    let mut absent = lane();
    absent.tailscaled = "/no/such/tailscaled".to_string();
    refresh_tailscaled(&mut report, &runner, &absent);
    assert_eq!(report.failures(), 0);
    assert!(runner.calls().is_empty(), "{:?}", runner.calls());
}

#[test]
fn a_tailscaled_that_matches_the_system_copy_is_not_reinstalled() {
    // No needless weekly VPN restart, and no privileged call at all in an
    // ordinary week.
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    refresh_tailscaled(&mut report, &runner, &lane());
    assert_eq!(report.failures(), 0);
    assert_eq!(
        runner.calls(),
        vec![vec![CMP, "-s", "/bin/sh", SYSTEM_TAILSCALED]]
    );
}

#[test]
fn a_tailscaled_that_moved_is_installed_into_the_system_daemon() {
    // `cmp` answering non-zero is the whole signal: the daemon is running
    // a build the upgrade replaced.
    let runner = ScriptedRunner::new(&[&[CMP, "-s", "/bin/sh", SYSTEM_TAILSCALED]]);
    let mut report = report();
    refresh_tailscaled(&mut report, &runner, &lane());
    assert_eq!(report.failures(), 0);
    assert_eq!(
        runner.calls().last(),
        Some(&vec![
            SUDO.to_string(),
            "-n".to_string(),
            "/bin/sh".to_string(),
            "install-system-daemon".to_string(),
        ])
    );
}

#[test]
fn a_converge_tool_that_will_not_run_is_a_failed_step_naming_what_to_do_about_it() {
    // Deleting the guard leaves the step failing anyway, since nothing can
    // exec a path that is not there. What the guard is for is the
    // SENTENCE: the record has to name the consequence and the fix, not
    // read "No such file or directory".
    let runner = ScriptedRunner::new(&[&["/b/converge"]]);
    let mut report = report();
    converge_osquery(&mut report, &runner, &lane());
    assert_eq!(report.failures(), 1);
    assert!(said(&report, "osquery config converge"), "{report:?}");
    assert!(said(&report, "may be the vendor default"), "{report:?}");
    assert!(said(&report, "run chezmoi apply"), "{report:?}");
}

#[test]
fn a_converge_nobody_configured_is_a_failed_step_rather_than_a_silent_skip() {
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    let mut unconfigured = lane();
    unconfigured.osquery_converge = Vec::new();
    converge_osquery(&mut report, &runner, &unconfigured);
    assert_eq!(report.failures(), 1);
    assert!(said(&report, "may be the vendor default"), "{report:?}");
    assert!(runner.calls().is_empty(), "{:?}", runner.calls());
}

#[test]
fn a_converge_that_ran_clean_is_the_quiet_week_it_is_meant_to_be() {
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    converge_osquery(&mut report, &runner, &lane());
    assert_eq!(report.failures(), 0);
    assert_eq!(runner.calls(), vec![vec!["/b/converge"]]);
}

#[test]
fn converge_argv_keeps_spaces_and_shell_text_as_literal_arguments() {
    use crate::LaneAdapter;
    let lane = BrewLane::parse(
        "lanes.brew",
        "osquery_converge = ['/home with space/posture', 'converge', '$(touch must-not-run)']"
            .parse()
            .unwrap(),
    )
    .unwrap();
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    converge_osquery(&mut report, &runner, &lane);
    assert_eq!(report.failures(), 0);
    assert_eq!(
        runner.calls(),
        [vec![
            "/home with space/posture",
            "converge",
            "$(touch must-not-run)"
        ]]
    );
}

#[test]
fn no_mas_manifest_published_is_a_clean_skip_rather_than_a_weekly_failure() {
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    let mut unpublished = lane();
    unpublished.mas_manifest = "/no/such/mas.Brewfile".to_string();
    mas_declarations(&mut report, &runner, &unpublished, Duration::from_secs(180));
    assert_eq!(report.failures(), 0);
    assert!(said(&report, "nothing to install"), "{report:?}");
    assert!(runner.calls().is_empty(), "{:?}", runner.calls());
}

#[test]
fn an_empty_manifest_declares_nothing_and_reads_the_same_as_an_absent_one() {
    // The apply publishes the file whether or not anything is declared,
    // so EXISTS is not the question. `brew bundle` on an empty Brewfile
    // would succeed and say nothing, which is a step the record carries
    // for work that could not have happened.
    let manifest = std::env::temp_dir().join(format!(
        "uu-brew-empty-manifest-{}.Brewfile",
        std::process::id()
    ));
    fs::write(&manifest, "").expect("an empty manifest");
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    let mut empty = lane();
    empty.mas_manifest = manifest.to_string_lossy().to_string();
    mas_declarations(&mut report, &runner, &empty, Duration::from_secs(180));
    let _ = fs::remove_file(&manifest);
    assert_eq!(report.failures(), 0);
    assert!(said(&report, "nothing to install"), "{report:?}");
    assert!(runner.calls().is_empty(), "{:?}", runner.calls());
}

#[test]
fn a_published_mas_manifest_is_installed_under_the_bound() {
    let runner = ScriptedRunner::new(&[]);
    let mut report = report();
    mas_declarations(&mut report, &runner, &lane(), Duration::from_secs(180));
    assert_eq!(report.failures(), 0);
    assert_eq!(
        runner.deadlines(),
        vec![(
            vec![
                "/b/brew".to_string(),
                "bundle".to_string(),
                "--no-upgrade".to_string(),
                "--file=/etc/hosts".to_string(),
            ],
            Duration::from_secs(180)
        )]
    );
}
