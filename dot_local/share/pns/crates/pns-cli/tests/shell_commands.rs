mod shell_support;
use shell_support::Fixture;

#[test]
fn the_marker_is_present_while_a_tracked_command_runs_and_gone_once_it_ends() {
    let fixture = Fixture::new("marker-lifecycle");
    let output = fixture.run(&["begin", "--command", "cargo build --release"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.marker().exists());
    let output = fixture.run(&[
        "end",
        "--command",
        "cargo build --release",
        "--exit",
        "0",
        "--elapsed",
        "3",
    ]);
    assert!(output.status.success());
    assert!(!fixture.marker().exists());
    assert!(!fixture.root.join("hermes.event").exists());
}

#[test]
fn shell_end_uses_the_existing_event_route_and_only_the_command_name() {
    let fixture = Fixture::new("end-event");
    let output = fixture.run(&[
        "end",
        "--command",
        "cargo build --secret invisible",
        "--exit",
        "7",
        "--elapsed",
        "300",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.marker().exists());
    let event = fixture.event();
    assert_eq!(event["detail"], "cargo (300s, exit 7)");
    assert_eq!(event["agent"], "shell");
    assert_eq!(event["state"], "failed");
    assert_eq!(event["project"], "logical project");
    assert_eq!(event["pane"], "t1:p2");
}

#[test]
fn the_marker_this_shell_writes_is_named_for_this_shells_own_pid() {
    let f = Fixture::new("own-pid");
    f.begin("cargo build");
    assert!(f.marker().exists());
    assert_eq!(
        std::fs::read_dir(f.root.join("state/lights-shell"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn a_command_that_failed_still_clears_the_marker() {
    let f = Fixture::new("failed-clear");
    f.begin("cargo build");
    assert!(
        f.run(&[
            "end",
            "--command",
            "cargo build",
            "--exit",
            "1",
            "--elapsed",
            "3"
        ])
        .status
        .success()
    );
    assert!(!f.marker().exists());
}

#[test]
fn a_short_command_in_one_pane_leaves_another_panes_marker_alone() {
    let f = Fixture::new("two-panes");
    let pane = shell_support::Pane::begin(&f);
    let marker = f.root.join(format!("state/lights-shell/{}", pane.pid));
    assert!(marker.exists());
    f.begin("ls");
    assert!(
        f.run(&["end", "--command", "ls", "--exit", "0", "--elapsed", "1"])
            .status
            .success()
    );
    assert!(!f.marker().exists());
    assert!(marker.exists());
}

#[test]
fn a_state_directory_that_cannot_be_created_costs_a_marker_never_the_command() {
    let f = Fixture::new("blocked-state");
    std::fs::write(f.root.join("blocked"), "not a directory").unwrap();
    let mut command = f.command();
    command.env("PNS_STATE_DIR", f.root.join("blocked/state"));
    let output = shell_support::capture(command.args([
        "shell",
        "begin",
        "--pid",
        &std::process::id().to_string(),
        "--command",
        "cargo build",
    ]));
    assert_eq!(output.status.code(), Some(1));
    assert!(!f.root.join("state").exists());
    // The Bash preexec successor separately proves it ignores this refusal.
}

#[test]
fn the_marker_is_one_epoch_line_in_a_directory_readable_by_nobody_else() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("private-epoch");
    let before = pns_adapters::now_secs().unwrap();
    f.begin("cargo build");
    let text = std::fs::read_to_string(f.marker()).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with('\n'));
    let epoch: u64 = text.trim().parse().unwrap();
    assert!(epoch >= before && epoch <= pns_adapters::now_secs().unwrap());
    assert_eq!(
        std::fs::metadata(f.marker()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(f.marker().parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn invalid_or_unowned_shell_pids_never_touch_state_or_notify() {
    let f = Fixture::new("pid-refusal");
    for pid in ["0", "1", "-1", "2147483648", "2"] {
        let output = shell_support::capture(f.command().args([
            "shell",
            "begin",
            "--pid",
            pid,
            "--command",
            "cargo build",
        ]));
        assert!(!output.status.success(), "{pid}");
        let output = shell_support::capture(f.command().args([
            "shell",
            "end",
            "--pid",
            pid,
            "--command",
            "cargo build",
            "--exit",
            "0",
            "--elapsed",
            "300",
        ]));
        assert!(!output.status.success(), "{pid}");
    }
    assert!(!f.root.join("state").exists());
    assert!(!f.root.join("hermes.event").exists());
    let pane = shell_support::Pane::begin(&f);
    let path = f.root.join(format!("state/lights-shell/{}", pane.pid));
    let before = std::fs::read(&path).unwrap();
    let output = shell_support::capture(f.command().args([
        "shell",
        "end",
        "--pid",
        &pane.pid.to_string(),
        "--command",
        "cargo build",
        "--exit",
        "0",
        "--elapsed",
        "300",
    ]));
    assert!(!output.status.success());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn end_clears_before_detached_delivery_and_cannot_erase_the_next_begin() {
    let f = Fixture::new("end-order");
    f.begin("first build");
    std::fs::write(
        f.root.join("channels/hermes.sh"),
        "#!/bin/sh\ncat >\"$HOME/hermes.event\"\n",
    )
    .unwrap();
    assert!(
        f.run(&[
            "end",
            "--command",
            "first build",
            "--exit",
            "0",
            "--elapsed",
            "30"
        ])
        .status
        .success()
    );
    assert!(!f.marker().exists());
    f.begin("second build");
    let text = std::fs::read_to_string(f.marker()).unwrap();
    let event = f.event();
    assert_eq!(event["detail"], "first (30s)");
    assert_eq!(std::fs::read_to_string(f.marker()).unwrap(), text);
}

#[test]
fn every_interactive_tui_is_skipped_by_both_real_commands() {
    let f = Fixture::new("real-tui");
    for name in [
        "vim", "nvim", "less", "man", "top", "btop", "ssh", "herdr", "claude", "hermes", "codex",
        "fzf",
    ] {
        f.begin(name);
        assert!(!f.marker().exists(), "{name}");
        let output = f.run(&["end", "--command", name, "--exit", "0", "--elapsed", "300"]);
        assert!(output.status.success());
    }
    assert!(!f.root.join("state").exists());
    assert!(!f.root.join("hermes.event").exists());
}

#[test]
fn failed_marker_removal_does_not_suppress_the_command_report() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("unlink-refusal");
    f.begin("cargo build");
    let directory = f.marker().parent().unwrap().to_path_buf();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let output = f.run(&[
        "end",
        "--command",
        "cargo build",
        "--exit",
        "0",
        "--elapsed",
        "30",
    ]);
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(output.status.success());
    assert!(f.marker().exists());
    assert_eq!(f.event()["detail"], "cargo (30s)");
}
