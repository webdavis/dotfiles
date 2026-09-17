mod support;

use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use support::Home;

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

/// A LIVENESS BOUND, NOT A MEASUREMENT: nothing below reads the elapsed time,
/// and every passing wait here ends on its own event in a few milliseconds.
/// Reaching this bound means a fixture process never arrived, which is a
/// failure whatever the clock says. Two seconds was a wall-clock budget for a
/// `uu` run spawning a child that spawns a grandchild while the operator's
/// other agent lanes compile.
const LIVENESS_BOUND: Duration = Duration::from_secs(15);

fn until(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(start.elapsed() < LIVENESS_BOUND, "fixture timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

// Invoked only as the command lane's owned child, never as a normal test.
#[test]
#[ignore]
fn child_fixture() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    // SAFETY: the handler only stores a lock-free flag and remains valid until exit.
    unsafe { libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t) };
    let grandchild = std::env::var_os("UU_GRANDCHILD").is_some();
    let mut child = (!grandchild).then(|| {
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child_fixture", "--ignored"])
            .env("UU_GRANDCHILD", "1")
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    });
    let role = if grandchild { "grandchild" } else { "child" };
    fs::write(home.join(role), std::process::id().to_string()).unwrap();
    // SAFETY: getpgrp only observes this fixture process.
    fs::write(
        home.join(format!("{role}-group")),
        unsafe { libc::getpgrp() }.to_string(),
    )
    .unwrap();
    until(|| STOP.load(Ordering::Relaxed) || home.join("release").exists());
    if STOP.load(Ordering::Relaxed) {
        let lock = File::open(home.join(".local/state/uu/run.lock")).unwrap();
        // SAFETY: this is the fixture's own open lock descriptor.
        let held = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0;
        fs::write(home.join(format!("{role}-lock-held")), held.to_string()).unwrap();
        fs::write(home.join(format!("{role}-stopped")), "yes").unwrap();
    }
    if let Some(child) = child.as_mut() {
        child.wait().unwrap();
    }
}

/// One marker file's CONTENTS, waited for rather than sampled. The child and
/// the grandchild write four files in their own orders, and `fs::write`
/// publishes the path before the bytes land, so an existence check can hand
/// back an empty string or a missing file from a process that is still
/// writing. Waiting on the value is the event the assertions actually need.
fn marker(home: &Path, name: &str) -> String {
    let path = home.join(name);
    let mut text = String::new();
    until(|| {
        text = fs::read_to_string(&path).unwrap_or_default();
        !text.is_empty()
    });
    text
}

fn interrupted(signal: i32, name: &str) {
    let home = Home::new(name);
    let original_marker = if signal == libc::SIGTERM {
        let home =
            home.with_config("[lanes.seed]\ntype = \"command\"\nrun = [\"/usr/bin/true\"]\n");
        assert!(home.uu(&["run"]).status.success());
        let marker = fs::read_to_string(home.marker()).unwrap();
        (home, Some(marker))
    } else {
        (home, None)
    };
    let (home, original_marker) = original_marker;
    let args = format!(
        "{:?}",
        [
            std::env::current_exe().unwrap().to_str().unwrap(),
            "--exact",
            "child_fixture",
            "--ignored",
        ]
    );
    let late = home.write_stub("late", "touch \"$HOME/late-ran\"\n");
    let alert = home.write_stub("alert", "touch \"$HOME/alert-ran\"\n");
    let home = home.with_config(&format!(
        "[lanes.first]\ntype = \"command\"\nrun = {args}\n\
         [lanes.later]\ntype = \"command\"\nrun = [{late:?}]\n\
         [alerts]\nbinary = {alert:?}\n"
    ));
    let mut run = Command::new(env!("CARGO_BIN_EXE_uu"))
        .arg("run")
        .env("HOME", &home.dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let group = marker(&home.dir, "child-group");
    assert_eq!(group, marker(&home.dir, "child"));
    assert_eq!(group, marker(&home.dir, "grandchild-group"));
    assert_ne!(
        group,
        run.id().to_string(),
        "producer joined uu's process group"
    );
    let log = home.dir.join(".local/log/uu/uu.log");
    let before = fs::read_to_string(&log).unwrap();
    // SAFETY: run is this test's unreaped child, never a live updater.
    assert_eq!(unsafe { libc::kill(run.id() as i32, signal) }, 0);
    until(|| run.try_wait().unwrap().is_some());
    // Release the old implementation's surviving fixtures before asserting.
    fs::write(home.dir.join("release"), "yes").unwrap();
    let status = run.wait().unwrap();
    assert!(
        before.contains("uu: run started"),
        "no durable start: {before}"
    );
    assert_eq!(status.code(), Some(128 + signal));
    for role in ["child", "grandchild"] {
        assert!(
            home.dir.join(format!("{role}-stopped")).exists(),
            "{role} survived"
        );
        assert_eq!(
            fs::read_to_string(home.dir.join(format!("{role}-lock-held"))).unwrap(),
            "true",
            "lock released before {role} cleanup"
        );
    }
    let after = fs::read_to_string(log).unwrap();
    assert!(after.contains("uu: run interrupted"), "{after}");
    assert_eq!(
        after.matches("=== done,").count(),
        usize::from(original_marker.is_some()),
        "interruption reported success: {after}"
    );
    assert_eq!(fs::read_to_string(home.marker()).ok(), original_marker);
    assert!(!home.dir.join("late-ran").exists());
    assert!(!home.dir.join("alert-ran").exists());
    let lock = File::open(home.dir.join(".local/state/uu/run.lock")).unwrap();
    // SAFETY: this descriptor belongs to the fixture and uu has exited.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
}

#[test]
fn sigint_cleans_owned_children_before_unlocking_and_records_interruption() {
    interrupted(libc::SIGINT, "interrupt-int");
}

#[test]
fn sigterm_cleans_owned_children_before_unlocking_and_records_interruption() {
    interrupted(libc::SIGTERM, "interrupt-term");
}
