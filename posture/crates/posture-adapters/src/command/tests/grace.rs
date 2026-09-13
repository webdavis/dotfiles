use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn the_deadline_sends_term_before_kill_and_retains_timeout_outcome() {
    let path = std::env::temp_dir().join(format!(
        "posture-term-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut runner = SystemRunner::per_command(Duration::from_millis(60))
        .with_termination_grace(Duration::from_millis(30));
    let start = Instant::now();
    let result = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            "-c".as_ref(),
            "trap 'printf term >\"$1\"; exit 0' TERM; while :; do :; done".as_ref(),
            "fixture".as_ref(),
            path.as_os_str(),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(result, Err(InspectionFailure::TimedOut));
    assert_eq!(fs::read(&path).ok().as_deref(), Some(b"term".as_slice()));
    assert!(start.elapsed() < Duration::from_millis(350));
}

#[test]
fn term_ignoring_child_and_grandchild_are_killed_and_reaped_under_the_bound() {
    let path = std::env::temp_dir().join(format!(
        "posture-group-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut runner = SystemRunner::per_command(Duration::from_millis(70))
        .with_termination_grace(Duration::from_millis(25));
    let start = Instant::now();
    let result = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            "-c".as_ref(),
            "trap '' TERM; /bin/sleep 600 & printf '%s\\n%s\\n' \"$$\" \"$!\" >\"$1\"; wait"
                .as_ref(),
            "fixture".as_ref(),
            path.as_os_str(),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    let pids: Vec<i32> = fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|pid| pid.parse().unwrap())
        .collect();
    let until = Instant::now() + Duration::from_millis(80);
    loop {
        // Only this fixture's recorded, owned processes are queried or cleaned up.
        let live: Vec<_> = pids
            .iter()
            .filter(|pid| unsafe { libc::kill(**pid, 0) == 0 })
            .collect();
        if live.is_empty() {
            break;
        }
        if Instant::now() >= until {
            for pid in live {
                unsafe {
                    libc::kill(*pid, libc::SIGKILL);
                }
            }
            panic!("owned descendants survived timeout");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(result, Err(InspectionFailure::TimedOut));
    assert!(start.elapsed() >= Duration::from_millis(95));
    assert!(start.elapsed() < Duration::from_millis(350));
}

#[test]
fn graceful_runner_keeps_healthy_exit_status_and_supplies_stdin_eof() {
    let mut runner = SystemRunner::per_command(Duration::from_millis(100))
        .with_termination_grace(Duration::from_secs(2));
    let output = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            "-c".as_ref(),
            "if read value; then exit 1; fi; printf eof; exit 7".as_ref(),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(
        output,
        Ok(CommandOutput {
            bytes: b"eof".to_vec(),
            exit: 7
        })
    );
}
