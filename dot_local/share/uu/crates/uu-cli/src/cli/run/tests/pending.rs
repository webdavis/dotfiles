use super::{Clock, Duration, Fixture, Observed, RunOutcome, execute};
use uu_adapters::ConsoleRunPresentation;
use uu_application::{RunHeader, RunPresentation};

#[test]
fn a_pending_command_lane_keeps_its_output_and_advances_the_marker() {
    let fixture = Fixture::new("pending-marker");
    let program = fixture.stub(
        "cat >/dev/null\nprintf 'two updates waiting\\n'\nprintf 'operator must accept pins\\n' >&2\nexit 100\n",
    );
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
            None,
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    let reports = observed.reports.borrow();
    assert_eq!(
        reports[0].failures(),
        0,
        "pending is successful work: {:?}",
        reports[0]
    );
    let header = RunHeader {
        host: "fixture-host".into(),
        started_iso: "fixture-start".into(),
        gap: "fixture-gap".into(),
    };
    let detail = ConsoleRunPresentation.write_record(&header, &reports);
    assert!(detail.contains("mine: pending"), "{detail}");
    assert!(detail.contains("two updates waiting"), "{detail}");
    assert!(detail.contains("operator must accept pins"), "{detail}");
    assert!(
        detail.contains("0 failure(s), 0 deferred, 1 pending"),
        "{detail}"
    );
    assert_eq!(
        uu_adapters::read_marker(&fixture.marker()),
        uu_domain::Marker::Recorded {
            epoch: 2_000,
            iso: "1970-01-01T00:33:20Z".into()
        }
    );
}

#[test]
fn a_command_lane_accepts_its_own_escalation_threshold() {
    let fixture = Fixture::new("pending-threshold");
    let config = fixture.load(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"/fixture/updater\"]\nescalate_after_runs = 2\n",
        crate::registrations::LANES,
    );
    assert!(
        config.is_ok(),
        "positive threshold must be read before the typed parser: {config:?}"
    );
}

#[test]
fn the_parsed_pending_threshold_uses_its_own_file_and_resets_after_completed_work() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("pending-lifecycle");
    let calls = fixture.dir.join("alarm-calls");
    let engine = fixture.dir.join("alarm-engine");
    std::fs::write(
        &engine,
        format!("#!/bin/sh\nset -eu\nprintf 'alarm\\n' >> {calls:?}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();
    let program = fixture.stub("cat >/dev/null\nexit 100\n");
    let config = fixture.load(&format!("[alerts]\nbinary = {engine:?}\n[lanes.mine]\ntype = \"command\"\nrun = [{program:?}]\nescalate_after_runs = 2\ndeadline_secs = 1\n"), crate::registrations::LANES).unwrap();
    let state = fixture.dir.join(".local/state/uu/lanes/mine");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("streak"), "2\n").unwrap();
    for (exit, pending, alarms) in [
        (100, 1, 0),
        (100, 2, 1),
        (100, 3, 1),
        (0, 0, 1),
        (100, 1, 1),
    ] {
        fixture.stub(&format!("cat >/dev/null\nexit {exit}\n"));
        assert_eq!(
            execute(
                fixture.home(),
                &config,
                None,
                Clock::new(Duration::ZERO),
                &Observed::default()
            ),
            RunOutcome::Completed
        );
        assert_eq!(
            std::fs::read_to_string(state.join("pending")).unwrap(),
            format!("{pending}\n")
        );
        assert_eq!(
            std::fs::read_to_string(state.join("streak")).unwrap(),
            "0\n"
        );
        let lines = match std::fs::read_to_string(&calls) {
            Ok(text) => text.lines().count(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => panic!("{error}"),
        };
        assert_eq!(lines, alarms);
        assert!(fixture.marker().exists());
    }
}
