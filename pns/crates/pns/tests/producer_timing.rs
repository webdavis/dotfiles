mod support;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use support::{Sandbox, stderr, stdout};

/// A FIXTURE BUDGET, NOT AN ASSERTION. Nothing in this file is about how long
/// the work may take; this bound exists only so a genuine hang fails the run
/// instead of wedging it.
///
/// The budgets it replaced were in the hundreds of milliseconds, which held on
/// an idle machine and failed on a loaded CI runner: spawning a real process
/// there can take longer than the whole old budget, and the failure then names
/// whatever the child had not finished rather than naming the budget. A
/// passing run never waits this long, because the wait ends when the work does.
const FIXTURE_BUDGET: Duration = Duration::from_secs(30);

fn command(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.pns_stateful();
    for (key, suffix) in [
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state-root"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
        ("XDG_CONFIG_DIRS", "config-dirs"),
        ("XDG_DATA_DIRS", "data-dirs"),
        ("CLAUDE_CONFIG_DIR", "claude"),
        ("TMPDIR", "tmp"),
        ("TMP", "tmp"),
        ("TEMP", "tmp"),
    ] {
        let path = sandbox.path(suffix);
        std::fs::create_dir_all(&path).unwrap();
        command.env(key, path);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    command
}

fn run(command: &mut Command) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A FIXTURE BUDGET, NOT AN ASSERTION. Nothing here is about how long the work
    // may take; this bound exists only so a genuine hang fails the run instead of
    // wedging it. It was a tenth of a second short of a second, which held on an idle machine and failed on a
    // loaded CI runner, where spawning a real process can take longer than the
    // whole budget. A passing run never waits this long: the wait ends when the
    // work does.
    let deadline = Instant::now() + FIXTURE_BUDGET;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("producer command exceeded its fixture budget");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn version_is_one_bare_semver_line_without_config_or_probes() {
    for flag in ["--version", "-V"] {
        let sandbox = Sandbox::without_config(&format!("version-{flag}"));
        let output = run(command(&sandbox)
            .args([flag])
            .env("PATH", sandbox.path("absent-bin")));
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(stdout(&output), concat!(env!("CARGO_PKG_VERSION"), "\n"));
        assert!(output.stderr.is_empty());
        assert!(!sandbox.state().exists());
        assert!(!sandbox.fired("hermes"));
    }
}

#[test]
fn elapsed_below_thirty_is_silent_before_state_or_delivery() {
    for seconds in [0, 10, 29] {
        for scope in [&[][..], &["--scope", "local_only"][..]] {
            let sandbox = Sandbox::new(&format!("elapsed-quiet-{seconds}"));
            let output = run(command(&sandbox)
                .args([
                    "send",
                    "--producer",
                    "nvim",
                    "--state",
                    "done",
                    "--elapsed",
                    &format!("{seconds}s"),
                ])
                .args(scope));
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stdout.is_empty(), "{}", stdout(&output));
            assert!(output.stderr.is_empty(), "{}", stderr(&output));
            assert!(!sandbox.fired("hermes"));
            assert!(!sandbox.state().exists());
        }
    }
}

fn assert_elapsed_tiers(seconds: &[u64]) {
    for &seconds in seconds {
        let sandbox = Sandbox::new(&format!("elapsed-tier-{seconds}"));
        let output = run(command(&sandbox).args([
            "send",
            "--producer",
            "nvim",
            "--state",
            "failed",
            "--detail",
            "neotest: owned",
            "--elapsed",
            &format!("{seconds}s"),
        ]));
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(
            sandbox.event("hermes")["detail"],
            format!("neotest: owned ({seconds}s)")
        );
        let db = rusqlite::Connection::open_with_flags(
            sandbox.path("state/pns.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let record: String = db
            .query_row(
                "SELECT line FROM decisions ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let expected = if seconds >= 300 {
            "long_running=yes"
        } else {
            "long_running=no"
        };
        assert!(record.contains(expected), "{record}");
    }
}

#[test]
fn elapsed_from_thirty_uses_normal_delivery_and_renders_the_duration() {
    assert_elapsed_tiers(&[30, 31, 35]);
}

#[test]
fn elapsed_from_three_hundred_selects_the_long_running_tier() {
    assert_elapsed_tiers(&[299, 300, 301]);
}

#[test]
fn elapsed_rejects_a_bare_number_and_every_other_non_duration() {
    // A BARE NUMBER IS THE ONE THAT MATTERS: `90` reads as seconds to one
    // caller and minutes to the next, so it is refused rather than guessed.
    for value in [
        "",
        "30",
        "0",
        "-1",
        "+30",
        "1.5",
        "NaN",
        "30 s",
        "30x",
        "721h",
        "18446744073709551616",
        "--scope",
    ] {
        let sandbox = Sandbox::new(&format!("elapsed-invalid-{}", value.replace('/', "_")));
        let mut cmd = command(&sandbox);
        cmd.args(["send", "--producer", "nvim", "--elapsed"]);
        if !value.is_empty() {
            cmd.arg(value);
        }
        let output = run(&mut cmd);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{value:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("pns: --elapsed"),
            "{value:?}: {}",
            stderr(&output)
        );
        assert!(!sandbox.fired("hermes"));
        assert!(!sandbox.state().exists());
    }
}

#[test]
fn long_running_is_refused_as_retired_whether_or_not_elapsed_is_also_given() {
    // pns derives the tier from `--elapsed` alone now, so this flag is
    // refused outright rather than accepted or silently dropped.
    for args in [
        &["send", "--elapsed", "35s", "--long-running"][..],
        &["send", "--long-running", "--elapsed", "35s"][..],
        &["send", "--long-running"][..],
    ] {
        let sandbox = Sandbox::new(&format!("long-running-retired-{}", args.join("-")));
        let output = run(command(&sandbox).args(args));
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "pns: --long-running was replaced by --elapsed\n"
        );
        assert!(!sandbox.fired("hermes"));
        assert!(!sandbox.state().exists());
    }
}

#[test]
fn help_still_wins_over_elapsed_refusal_without_delivery() {
    let sandbox = Sandbox::new("elapsed-help");
    let output = run(command(&sandbox).args(["send", "--elapsed", "bad", "--help"]));
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("--elapsed <duration>"));
    assert!(output.stderr.is_empty());
    assert!(!sandbox.state().exists());
}

#[test]
fn elapsed_still_obeys_the_presence_gate() {
    let sandbox = Sandbox::new("elapsed-presence");
    let mut command = command(&sandbox);
    command.env("PNS_PHONE_INPUT_MAX_AGE", "0s");
    sandbox.stub_herdr(&mut command, true);
    let output = run(command.args([
        "send",
        "--producer",
        "nvim",
        "--state",
        "done",
        "--elapsed",
        "35s",
        "--pane",
        "t1:p2",
    ]));
    assert_eq!(output.status.code(), Some(0));
    assert!(!sandbox.fired("phone"));
    assert!(!sandbox.fired("banner"));
    assert!(
        sandbox.fired("hermes"),
        "the existing recording destination remains enabled"
    );
}

#[test]
fn elapsed_flag_is_protected_and_empty_detail_is_rendered() {
    let sandbox = Sandbox::new("elapsed-protected");
    let output = run(command(&sandbox).args(["send", "--detail", "--elapsed", "35s"]));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stderr(&output),
        "pns: --detail given without a value; ignoring\n"
    );
    assert_eq!(sandbox.event("hermes")["detail"], "35s");
}

#[test]
fn invalid_elapsed_is_not_erased_by_a_later_valid_value() {
    let sandbox = Sandbox::new("elapsed-invalid-then-valid");
    let output = run(command(&sandbox).args(["send", "--elapsed", "bad", "--elapsed", "35s"]));
    assert_eq!(output.status.code(), Some(2));
    assert!(!sandbox.state().exists());
    assert!(!sandbox.fired("hermes"));
}

fn decision_line(sandbox: &Sandbox) -> String {
    rusqlite::Connection::open_with_flags(
        sandbox.path("state/pns.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .query_row(
        "SELECT line FROM decisions ORDER BY seq DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

fn send_json(sandbox: &Sandbox, request: &str) -> Output {
    use std::io::Write;
    let mut child = command(sandbox)
        .args(["send", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// ONE DURATION, ONE TIER, WHICHEVER PATH SPELLED IT. `--elapsed 90s` and
/// `"elapsed": "90s"` are the same statement, so the tier they earn cannot
/// depend on which of the two a producer typed.
#[test]
fn one_duration_spelling_earns_one_tier_on_the_flag_path_and_the_json_path() {
    for (elapsed, expected) in [("90s", "long_running=no"), ("300s", "long_running=yes")] {
        let flags = Sandbox::new(&format!("duration-flags-{elapsed}"));
        let output = run(command(&flags).args([
            "send",
            "--producer",
            "nvim",
            "--state",
            "done",
            "--detail",
            "neotest: owned",
            "--elapsed",
            elapsed,
        ]));
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(flags.fired("hermes"));
        assert!(decision_line(&flags).contains(expected), "{elapsed}");

        let json = Sandbox::new(&format!("duration-json-{elapsed}"));
        let request = format!(
            r#"{{"schema":"pns.request/1","request_id":"nvim-{elapsed}","producer":"nvim","event":"finished","state":"done","detail":"neotest: owned","elapsed":"{elapsed}"}}"#
        );
        let output = send_json(&json, &request);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(json.fired("hermes"));
        assert!(decision_line(&json).contains(expected), "{elapsed}");
    }
}

/// A BARE NUMBER IS REFUSED ON BOTH PATHS, with nothing delivered.
#[test]
fn a_bare_elapsed_number_is_refused_on_the_flag_path_and_the_json_path() {
    let flags = Sandbox::new("duration-bare-flags");
    let output = run(command(&flags).args(["send", "--producer", "nvim", "--elapsed", "90"]));
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(!flags.fired("hermes"));

    let json = Sandbox::new("duration-bare-json");
    let output = send_json(
        &json,
        r#"{"schema":"pns.request/1","request_id":"nvim-bare","producer":"nvim","event":"finished","state":"done","elapsed":"90"}"#,
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(!json.fired("hermes"));
}

/// The id a caller names is the one the submission is recorded under, which is
/// what makes a retried call the same page rather than a second one.
#[test]
fn the_request_id_a_caller_named_is_the_one_the_submission_is_recorded_under() {
    let sandbox = Sandbox::new("request-id-flag");
    let output = run(command(&sandbox).args([
        "send",
        "--producer",
        "nvim",
        "--state",
        "done",
        "--detail",
        "owned",
        "--request-id",
        "nvim-slice-eleven",
        "--session",
        "s-2026-09-17-a",
    ]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let (request_id, producer): (String, String) = rusqlite::Connection::open_with_flags(
        sandbox.path("state/pns.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .query_row(
        "SELECT request_id, producer FROM ledger_events ORDER BY seq DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap();
    assert_eq!(request_id, "nvim-slice-eleven");
    // THE CALLER'S OWN NAMESPACE, not a shared "pns" every caller would
    // collide in: the ledger key is (producer, request_id).
    assert_eq!(producer, "nvim");
}
