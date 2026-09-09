use super::*;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn a_replay_hold_is_adopted_after_the_owned_process_exits() {
    let expires = Instant::now() + Duration::from_millis(700);
    const CHILD_STATE: &str = "PNS_FILE_HOLD_CHILD_STATE";
    if let Some(state) = std::env::var_os(CHILD_STATE) {
        let moment = FileReturnMoment::new(PathBuf::from(state));
        let claim = moment
            .claim(Some(2_000), true)
            .expect("the child owns the moment");
        assert_eq!(claim.waiting[0].detail, "still owed");
        // The process exits without completing its replay.
        return;
    }
    let replay = Replay::new("replay-owner-exit", Attempt::Completed);
    let binary = std::env::current_exe().expect("the fixture binary");
    let mut child = Command::new(binary)
        .args(["--exact", "protocols::return_window::tests::process::a_replay_hold_is_adopted_after_the_owned_process_exits"])
        .env(CHILD_STATE, &replay.state)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .process_group(0).spawn().expect("the owned fixture child");
    let status = loop {
        if let Some(status) = child.try_wait().expect("the owned child's status") {
            break status;
        }
        if Instant::now() >= expires {
            // SAFETY: this newly spawned child owns the process group named
            // by its id. No other process-group identifier is accepted here.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.wait();
            panic!("owned replay fixture exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(status.success(), "the fixture child failed: {status}");
    assert_eq!(
        holds(&replay.state).len(),
        1,
        "process exit must leave the undelivered batch"
    );
    let claim = replay
        .moment
        .claim(Some(2_001), true)
        .expect("the abandoned batch is claimable");
    assert_eq!(claim.waiting.len(), 1);
    assert_eq!(claim.waiting[0].detail, "still owed");
    replay.moment.complete();
    assert!(holds(&replay.state).is_empty());
}
