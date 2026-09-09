use super::*;
use crate::SystemRunner;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

struct Engine {
    executable: PathBuf,
}
impl Engine {
    fn new(body: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "posture-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let executable = directory.join("engine");
        fs::write(&executable, format!("#!/bin/bash\nset -euo pipefail\n[[ $# == 2 && $1 == submit && $2 == --json ]]\n/bin/cat >\"$0.input\"\n{body}\n")).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        Self { executable }
    }
    fn producer(&self) -> PnsProducer<SystemRunner, Alarm> {
        PnsProducer::new(
            SystemRunner::new(Duration::from_millis(150)),
            self.executable.clone(),
            None,
            Alarm::default(),
        )
    }
}
#[test]
fn a_real_owned_engine_receives_one_complete_request_and_returns_its_committed_identity() {
    let engine = Engine::new(
        r#"printf '%s\n' '{"schema":"pns.result/1","request_id":"posture-d28d5af268c004d795ce0240f35f5218","status":"accepted","decision_id":"17","diagnostics":["ledger_committed"]}'"#,
    );
    let mut sut = engine.producer();
    assert_eq!(sut.submit(&alert()), Submission::Accepted);
    let input = fs::read(engine.executable.with_extension("input")).unwrap();
    assert_eq!(input.last(), Some(&b'\n'));
    let request = decode_request(&input).unwrap().request;
    assert_eq!(request.detail, "Security finding\nline one\nline two");
    assert!(sut.alarm.calls.is_empty());
}
#[test]
fn a_real_hung_engine_is_reaped_before_one_alarm_and_never_accepted() {
    let engine = Engine::new(
        r#"/bin/sh -c 'trap "" TERM; while :; do :; done' &
printf '%s\n%s\n' "$$" "$!" >"$0.ready"
wait"#,
    );
    let mut sut = engine.producer();
    let started = Instant::now();
    let result = sut.submit(&alert());
    let elapsed = started.elapsed();
    let pids: Vec<i32> = fs::read_to_string(engine.executable.with_extension("ready"))
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    let limit = Instant::now() + Duration::from_millis(80);
    let live = loop {
        // Only identifiers emitted by this test's owned child are inspected or cleaned up.
        let live: Vec<i32> = pids
            .iter()
            .copied()
            .filter(|pid| unsafe { libc::kill(*pid, 0) == 0 })
            .collect();
        if live.is_empty() || Instant::now() >= limit {
            break live;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    for pid in &live {
        unsafe {
            libc::kill(*pid, libc::SIGKILL);
        }
    }
    assert_eq!(result, Submission::NotAccepted(SubmissionFailure::TimedOut));
    assert_eq!(sut.alarm.calls.len(), 1);
    assert_eq!(pids.len(), 2);
    assert!(live.is_empty(), "engine descendants survived: {live:?}");
    assert!(
        elapsed < Duration::from_millis(350),
        "engine exceeded deadline: {elapsed:?}"
    );
}
