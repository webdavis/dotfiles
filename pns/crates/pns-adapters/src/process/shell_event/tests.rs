use super::*;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

/// Exactly what the producer must be handed.
const EXPECTED_ARGV: &str = "--agent\nshell\n--state\ndone\n--project\nproject\n--detail\ncargo (300s)\n--pane\nt1:p2\n--long-running\n";

/// How long the fixture waits for anything.
///
/// PATIENCE, not a measurement: every wait below is for a spawned shell to
/// reach its next line, and a launch that never detached fails these however
/// long they wait. It was 500 ms, which is a wall-clock budget for starting a
/// shell while the rest of the suite competes for the same CPU.
const PATIENT: Duration = Duration::from_secs(10);

#[test]
fn shell_delivery_detaches_before_the_destination_finishes() {
    let root = std::env::temp_dir().join(format!("shell-launch-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let binary = root.join("producer");
    std::fs::write(
        &binary,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$$" >'{0}/ready'
printf '%s\n' "$@" >'{0}/argv'
i=0
while [ ! -e '{0}/release' ] && [ "$i" -lt 2000 ]; do
  i=$((i + 1))
  sleep 0.005
done
printf done >'{0}/done'
"#,
            root.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let event =
        pns_domain::shell_event("cargo build", 0, 300, "project".into(), "t1:p2".into()).unwrap();
    spawn(binary, &event).unwrap();
    let deadline = Instant::now() + PATIENT;
    // WAIT FOR THE CONTENT, not for the name. A redirection creates the file
    // before it writes, so an existence test returned an empty or half-written
    // argv and the comparison below failed on a loaded machine.
    let mut argv = String::new();
    while argv != EXPECTED_ARGV {
        assert!(
            Instant::now() < deadline,
            "producer did not record its argv, last read {argv:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
        argv = std::fs::read_to_string(root.join("argv")).unwrap_or_default();
    }
    assert!(!root.join("done").exists(), "launch waited for delivery");
    let pid: i32 = std::fs::read_to_string(root.join("ready"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // The owned child is alive at the barrier; getpgid only observes its group.
    assert_eq!(unsafe { libc::getpgid(pid) }, pid);
    std::fs::write(root.join("release"), "").unwrap();
    while !root.join("done").exists() {
        assert!(Instant::now() < deadline, "producer did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
    // The production caller exits immediately; this longer-lived test owns
    // its actual child and reaps it rather than leaving a zombie in libtest.
    let mut status = 0;
    loop {
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            break;
        }
        assert!(Instant::now() < deadline, "producer was not reaped");
        std::thread::sleep(Duration::from_millis(2));
    }
}
