mod support;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use support::{Sandbox, stderr, stdout};

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
    let deadline = Instant::now() + Duration::from_millis(600);
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
        let sandbox = Sandbox::new(&format!("elapsed-quiet-{seconds}"));
        let output = run(command(&sandbox).args([
            "--agent",
            "nvim",
            "--state",
            "done",
            "--elapsed",
            &seconds.to_string(),
        ]));
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty(), "{}", stdout(&output));
        assert!(output.stderr.is_empty(), "{}", stderr(&output));
        assert!(!sandbox.fired("hermes"));
        assert!(!sandbox.state().exists());
    }
}

fn assert_elapsed_tiers(seconds: &[u64]) {
    for &seconds in seconds {
        let sandbox = Sandbox::new(&format!("elapsed-tier-{seconds}"));
        let output = run(command(&sandbox).args([
            "--agent",
            "nvim",
            "--state",
            "failed",
            "--detail",
            "neotest: owned",
            "--elapsed",
            &seconds.to_string(),
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
fn elapsed_rejects_malformed_missing_and_overflowing_seconds() {
    for value in [
        "",
        "-1",
        "+30",
        "1.5",
        "NaN",
        "18446744073709551616",
        "--local-only",
    ] {
        let sandbox = Sandbox::new(&format!("elapsed-invalid-{}", value.replace('/', "_")));
        let mut cmd = command(&sandbox);
        cmd.args(["--agent", "nvim", "--elapsed"]);
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
            stderr(&output).contains("--elapsed requires a nonnegative whole number of seconds")
        );
        assert!(!sandbox.fired("hermes"));
        assert!(!sandbox.state().exists());
    }
}

#[test]
fn elapsed_rejects_an_explicit_tier_in_either_order() {
    for args in [
        ["--elapsed", "35", "--long-running"],
        ["--long-running", "--elapsed", "35"],
    ] {
        let sandbox = Sandbox::new(&format!("elapsed-conflict-{}", args[0]));
        let output = run(command(&sandbox).args(args));
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "pns: --elapsed cannot be combined with --long-running\n"
        );
        assert!(!sandbox.fired("hermes"));
        assert!(!sandbox.state().exists());
    }
}

#[test]
fn help_still_wins_over_elapsed_refusal_without_delivery() {
    let sandbox = Sandbox::new("elapsed-help");
    let output = run(command(&sandbox).args(["--elapsed", "bad", "--help"]));
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("--elapsed <secs>"));
    assert!(output.stderr.is_empty());
    assert!(!sandbox.state().exists());
}

#[test]
fn legacy_events_keep_their_detail_and_explicit_long_running_tier() {
    let sandbox = Sandbox::new("elapsed-legacy");
    let output = run(command(&sandbox).args([
        "--agent",
        "shell",
        "--state",
        "done",
        "--detail",
        "build (305s)",
        "--long-running",
    ]));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(sandbox.event("hermes")["detail"], "build (305s)");
}

#[test]
fn elapsed_still_obeys_the_presence_gate() {
    let sandbox = Sandbox::new("elapsed-presence");
    let mut command = command(&sandbox);
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, true);
    let output = run(command.args([
        "--agent",
        "nvim",
        "--state",
        "done",
        "--elapsed",
        "35",
        "--pane",
        "t1:p2",
    ]));
    assert_eq!(output.status.code(), Some(0));
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("macos-banner"));
    assert!(
        sandbox.fired("hermes"),
        "the existing recording destination remains enabled"
    );
}

#[test]
fn elapsed_flag_is_protected_and_empty_detail_is_rendered() {
    let sandbox = Sandbox::new("elapsed-protected");
    let output = run(command(&sandbox).args(["--detail", "--elapsed", "35"]));
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
    let output = run(command(&sandbox).args(["--elapsed", "bad", "--elapsed", "35"]));
    assert_eq!(output.status.code(), Some(2));
    assert!(!sandbox.state().exists());
    assert!(!sandbox.fired("hermes"));
}
