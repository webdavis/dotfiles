use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn timed_probe(script: &'static str) {
    let directory = std::env::temp_dir().join(format!(
        "posture-probe-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).expect("private test directory");
    let ready = directory.join("ready");
    let child_ready = ready.clone();
    let (send, receive) = mpsc::channel();
    let started = Instant::now();
    std::thread::spawn(move || {
        let mut runner = SystemRunner::new(Duration::from_millis(40));
        let result = runner.run(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new(script),
                OsStr::new("fixture"),
                child_ready.as_os_str(),
            ],
            true,
        );
        let second = runner.run(Path::new("/fixture/no-second-probe"), &[], false);
        let _ = send.send((result, second));
    });
    let result = receive.recv_timeout(Duration::from_millis(350));
    let elapsed = started.elapsed();
    let pids: Vec<i32> = std::fs::read_to_string(&ready)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.parse().ok())
        .collect();
    // A killed descendant is reaped by the operating system after its parent exits.
    // Observe its disappearance under a separate short bound; a live mutant stays visible.
    let absent_by = Instant::now() + Duration::from_millis(80);
    let live = loop {
        let live: Vec<i32> = pids
            .iter()
            .copied()
            .filter(|pid| {
                // These identifiers came only from this test's owned ready file.
                unsafe { libc::kill(*pid, 0) == 0 }
            })
            .collect();
        if live.is_empty() || Instant::now() >= absent_by {
            break live;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    for pid in &live {
        // Cleanup also runs when a deadline or group-termination mutant fails.
        unsafe {
            libc::kill(-*pid, libc::SIGKILL);
            libc::kill(*pid, libc::SIGKILL);
        }
    }
    assert!(
        !pids.is_empty(),
        "the probe must have reached its ready record"
    );
    assert!(
        elapsed < Duration::from_millis(200),
        "the runner exceeded its budget: {elapsed:?}"
    );
    assert_eq!(
        result.expect("the independent harness deadline expired"),
        (
            Err(InspectionFailure::TimedOut),
            Err(InspectionFailure::TimedOut)
        )
    );
    assert!(
        live.is_empty(),
        "owned processes survived the returned timeout: {live:?}"
    );
}

#[test]
fn a_closed_output_pipe_does_not_remove_the_child_deadline() {
    timed_probe("printf '%s\\n' \"$$\" >\"$1\"; exec 1>&- 2>&-; while :; do :; done");
}

#[test]
fn timeout_terminates_the_owned_process_group() {
    timed_probe(
        "/bin/sh -c 'trap \"\" TERM; while :; do :; done' & printf '%s\\n%s\\n' \"$$\" \"$!\" >\"$1\"; wait",
    );
}

#[test]
fn continuous_output_cannot_extend_the_absolute_deadline() {
    timed_probe("printf '%s\\n' \"$$\" >\"$1\"; while :; do printf 'output'; done");
}
