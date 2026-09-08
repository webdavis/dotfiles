use super::*;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // SAFETY: the fixture owns this child's process group, created at spawn.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
            }
        }
        let _ = self.0.wait();
    }
}
fn competing_claim(retry: bool, name: &str) {
    const KEY: &str = "PNS_LEDGER_CLAIM_CHILD";
    let mut input = submission();
    input.legs.truncate(1);
    if let Some(path) = std::env::var_os(KEY) {
        let path = std::path::PathBuf::from(path);
        let store = SqliteStore::new(path.clone());
        if retry {
            assert!(store.claim_retry(lease(20, 50)).unwrap().is_some());
        } else {
            created(&store, &input);
        }
        std::fs::write(path.join("ready"), b"owned").unwrap();
        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        return;
    }
    let path = state();
    let store = SqliteStore::new(path.clone());
    let _database = store.connect().unwrap();
    if retry {
        created(&store, &input);
    }
    let mut child = Owned(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name])
            .env(KEY, &path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_millis(650);
    while !path.join("ready").exists() {
        assert!(
            Instant::now() < deadline,
            "owned claimant did not become ready"
        );
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "owned claimant exited before ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let now = if retry { 21 } else { 11 };
    assert!(
        store.claim_retry(lease(now, 60)).unwrap().is_none(),
        "live claimant excludes competing retry"
    );
    assert!(
        matches!(
            store.prepare(&input, lease(now, 60)).unwrap(),
            PreparedSubmission::Existing(_)
        ),
        "in-flight duplicate never dispatches"
    );
    drop(child);
    let recovered = store
        .claim_retry(lease(now, 60))
        .unwrap()
        .expect("process death must retain and release unknown work");
    assert_eq!(recovered.identity, input.identity);
    assert_eq!(recovered.event, input.event);
    assert_eq!(recovered.leg, input.legs[0]);
    let history = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(history.attempts.len(), if retry { 3 } else { 2 });
    assert!(
        history.attempts.iter().all(|attempt| matches!(
            attempt.completion,
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unknown,
                ..
            }
        )),
        "no acknowledgement may be invented after process death"
    );
    store
        .record(&recovered.claim, &acknowledged(), now)
        .unwrap();
    assert!(store.claim_retry(lease(100, 110)).unwrap().is_none());
}
#[test]
fn an_initial_claim_excludes_a_competing_process_and_survives_its_owners_death() {
    competing_claim(
        false,
        "persistence::sqlite::ledger::tests::processes::an_initial_claim_excludes_a_competing_process_and_survives_its_owners_death",
    );
}
#[test]
fn a_retry_claim_excludes_a_competing_process_and_survives_its_owners_death() {
    competing_claim(
        true,
        "persistence::sqlite::ledger::tests::processes::a_retry_claim_excludes_a_competing_process_and_survives_its_owners_death",
    );
}
