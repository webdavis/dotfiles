mod pending;
mod support;

use super::execute;
use std::time::{Duration, Instant};
use support::{Clock, Fixture, Observed};
use uu_adapters::{CommandLane, LaneRegistration};
use uu_application::RunOutcome;

const ALIAS: &[LaneRegistration] = &[LaneRegistration::new::<CommandLane>("fixture-command")];

#[test]
fn an_alternate_registration_reaches_execution_and_doctor_with_distinct_names() {
    let fixture = Fixture::new("registration");
    let program = fixture.stub("printf '%s\\n' \"$1\" \"$2\"\ncat\n");
    let config = fixture.load(&format!(
        "[lanes.chosen]\ntype = \"fixture-command\"\nrun = [{program:?}, \"first\", \"second argument\"]\ndeadline_secs = 7\n"
    ), ALIAS).unwrap();
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            Some("chosen"),
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    let reports = observed.reports.borrow();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.name, "chosen");
    assert_eq!(report.failures(), 0, "{report:?}");
    assert_eq!(&report.lines[..2], ["first", "second argument"]);
    let event = report.lines[2..].join("\n");
    assert!(event.contains("\"lane\":\"chosen\""), "{event}");
    assert_eq!(config.lanes["chosen"].deadline, Duration::from_secs(7));
    let lines = crate::cli::doctor::lanes(uu_adapters::style::Paint::Plain, &config);
    assert_eq!(lines[0], "  \u{b7} chosen: on (fixture-command)");
    assert!(
        lines[1].contains(&format!("{}, found", program.display())),
        "{lines:?}"
    );
}

#[test]
fn a_missing_registration_refuses_before_any_command_or_diagnostic_can_run() {
    let fixture = Fixture::new("missing-registration");
    let touched = fixture.dir.join("touched");
    let program = fixture.stub(&format!("touch {touched:?}\n"));
    let error = fixture
        .load(
            &format!("[lanes.chosen]\ntype = \"fixture-command\"\nrun = [{program:?}]\n"),
            crate::registrations::LANES,
        )
        .unwrap_err();
    assert_eq!(
        error.detail(),
        "lane `chosen` has type `fixture-command`, which is no lane type; this build serves brew, cargo, claude-plugins, command, herdr, npm, nvim-mason, nvim-parsers, nvim-plugins, nvim-smoke-test, rotate-logs, rustup, skills, uv"
    );
    assert!(!touched.exists());
    assert!(!fixture.marker().exists());
}

#[test]
fn the_production_registration_list_loads_and_runs_the_selected_command() {
    let fixture = Fixture::new("production-registration");
    let program = fixture.stub("cat >/dev/null\nprintf 'actually ran\\n'\n");
    let config = fixture
        .load(
            &format!("[lanes.mine]\ntype = \"command\"\nrun = [{program:?}]\n"),
            crate::registrations::LANES,
        )
        .unwrap();
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            Some("mine"),
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    assert_eq!(observed.reports.borrow()[0].lines, ["actually ran"]);
    assert_eq!(
        crate::cli::doctor::lanes(uu_adapters::style::Paint::Plain, &config)[0],
        "  \u{b7} mine: on (command)"
    );
}

#[test]
fn a_lane_that_outlives_its_deadline_fails_instead_of_holding_the_run_open() {
    // The parsed seven-second setting reaches the real runner. A nearly spent
    // run leaves 80ms for the pipe holder, without changing legal config units.
    // The retained pure policy case covers expiry of the lane's own budget;
    // here the record must distinguish that setting from the run's remainder.
    let fixture = Fixture::new("lane-deadline");
    let group = fixture.dir.join("group");
    let program = fixture.stub(&format!(
        "if [ \"$1\" != update ]; then exit 0; fi\nprintf '%s\\n' \"$$\" >{group:?}\nsleep 0.6 &\nexit 0\n"
    ));
    let config = fixture
        .load(
            &format!("[lanes.herdr]\nbinary = {program:?}\ndeadline_secs = 7\n"),
            crate::registrations::LANES,
        )
        .unwrap();
    let observed = Observed::default();
    let began = Instant::now();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            None,
            Clock::new(uu_domain::RUN_DEADLINE - Duration::from_millis(80)),
            &observed
        ),
        RunOutcome::Completed
    );
    let elapsed = began.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "pipe holder was not stopped: {elapsed:?}"
    );
    let reports = observed.reports.borrow();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].failures(), 1, "{reports:?}");
    let said = reports[0].lines.join("\n");
    assert!(said.contains("lane `herdr` was stopped at 80ms"), "{said}");
    assert!(said.contains("its own deadline_secs is 7s"), "{said}");
    assert!(said.contains("process group was killed"), "{said}");
    assert!(!fixture.marker().exists());
    let group: i32 = std::fs::read_to_string(group)
        .expect("child started")
        .trim()
        .parse()
        .unwrap();
    assert!(group > 1);
    // SAFETY: signal zero only asks whether the fixture's recorded group exists.
    let exists = unsafe { libc::kill(-group, 0) };
    assert_eq!(exists, -1, "owned group {group} survived");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
fn the_marker_stamps_when_the_run_finished_and_not_when_it_started() {
    // Distinct clock readings make the old start-versus-finish contract exact,
    // without spinning across a wall-clock second inside the child.
    let fixture = Fixture::new("finish-time");
    let program = fixture.stub("exit 0\n");
    let config = fixture
        .load(
            &format!("[lanes.herdr]\nbinary = {program:?}\n"),
            crate::registrations::LANES,
        )
        .unwrap();
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            None,
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    assert_eq!(*observed.epochs.borrow(), [1_000]);
    let marker = std::fs::read_to_string(fixture.marker()).expect("successful marker");
    assert_eq!(marker.split_whitespace().next(), Some("2000"), "{marker}");
    assert_eq!(observed.reports.borrow()[0].failures(), 0);
}

mod nvim;
