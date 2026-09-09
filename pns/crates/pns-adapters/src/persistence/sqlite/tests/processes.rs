use super::{SqliteStore, state};
use pns_application::Journal;
use pns_domain::EventArgs;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(super) struct Owned(pub(super) Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // SAFETY: this fixture spawned this child into its own process group.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
            }
        }
        let _ = self.0.wait();
    }
}

#[test]
fn a_second_process_cannot_write_during_a_transaction_and_a_killed_writer_leaves_no_partial_record()
{
    const STATE: &str = "PNS_SQLITE_TRANSACTION_CHILD";
    // FIXTURE PATIENCE, not a measurement: this bounds waits for a spawned
    // process to reach its next line, and a fixture that never gets there fails
    // however long this waits. It was 700 ms, a wall-clock budget for starting a
    // process while the rest of the suite competes for the same CPU.
    let expires = Instant::now() + Duration::from_secs(30);
    if let Some(path) = std::env::var_os(STATE) {
        let path = std::path::PathBuf::from(path);
        let store = SqliteStore::new(path.clone());
        store
            .transaction(|transaction| {
                transaction
                    .execute("INSERT INTO journal(line) VALUES ('uncommitted event')", [])?;
                std::fs::write(path.join("ready"), b"locked")?;
                // HELD UNTIL THE PARENT KILLS THIS PROCESS, which is what the
                // parent does once it has made its observations. The bound is
                // only so a fixture whose parent was interrupted exits on its
                // own. It was 350 ms, which made the hold a window the parent
                // had to fit three steps inside, and on a slower machine the
                // transaction was already over by the time it looked.
                let until = Instant::now() + Duration::from_secs(30);
                while Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err::<(), _>(rusqlite::Error::InvalidQuery.into())
            })
            .unwrap_err();
        return;
    }
    let state = state();
    let mut store = SqliteStore::new(state.clone());
    store.busy_timeout = Duration::from_millis(5);
    let _keep_wal_open = store.connect().unwrap();
    let mut child = Owned(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "persistence::sqlite::tests::processes::a_second_process_cannot_write_during_a_transaction_and_a_killed_writer_leaves_no_partial_record"])
        .env(STATE, &state).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .process_group(0).spawn().unwrap());
    while !state.join("ready").exists() {
        assert!(
            Instant::now() < expires,
            "owned database fixture did not acquire its transaction"
        );
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "owned database fixture exited before acquiring its transaction"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        Journal::read(&store).unwrap(),
        None,
        "uncommitted rows must remain invisible to another connection"
    );
    let started = Instant::now();
    assert!(
        store
            .record_journal(&EventArgs::default(), Some(2), None)
            .is_err(),
        "another process owns the write transaction"
    );
    // A MEASUREMENT, and it survives being generous: the writer above holds its
    // transaction for thirty seconds, so a write that waits for the lock rather
    // than for its own five millisecond budget blows through this by six times.
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the configured busy budget must bound the wait"
    );
    drop(child);
    assert_eq!(
        Journal::read(&store).unwrap(),
        None,
        "process death must not commit a partial record"
    );
    store
        .record_journal(
            &EventArgs {
                detail: "after crash".into(),
                ..EventArgs::default()
            },
            Some(3),
            None,
        )
        .unwrap();
    assert!(
        Journal::read(&store)
            .unwrap()
            .unwrap()
            .contains("after crash")
    );
}

#[test]
fn a_committed_journal_hold_is_adopted_after_its_owned_process_exits() {
    const KEY: &str = "PNS_SQLITE_HELD_CHILD";
    let expires = Instant::now() + Duration::from_millis(700);
    if let Some(path) = std::env::var_os(KEY) {
        let store = SqliteStore::new(path.into());
        store
            .record_journal(
                &EventArgs {
                    detail: "held before exit".into(),
                    ..EventArgs::default()
                },
                Some(1),
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .claim_return(Some(2), true)
                .unwrap()
                .unwrap()
                .waiting
                .len(),
            1
        );
        return;
    }
    let path = state();
    let store = SqliteStore::new(path.clone());
    let _open = store.connect().unwrap();
    let mut child = Owned(Command::new(std::env::current_exe().unwrap()).args(["--exact", "persistence::sqlite::tests::processes::a_committed_journal_hold_is_adopted_after_its_owned_process_exits"]).env(KEY, &path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn().unwrap());
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < expires,
            "owned claim fixture did not finish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(Journal::read(&store).unwrap(), None);
    let claim = store.claim_return(Some(3), true).unwrap().unwrap();
    assert_eq!(
        claim.since, None,
        "adoption retains the window that opened before the interrupted return"
    );
    assert_eq!(claim.waiting.len(), 1);
    assert_eq!(claim.waiting[0].detail, "held before exit");
    store.complete_return().unwrap();
    assert!(
        store
            .claim_return(Some(4), true)
            .unwrap()
            .unwrap()
            .waiting
            .is_empty()
    );
}
