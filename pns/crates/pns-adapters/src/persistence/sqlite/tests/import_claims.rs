use super::{SqliteStore, processes::Owned, state};
use pns_application::Journal;
use pns_domain::EventArgs;
use std::{
    fs,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn dead_owner() -> u32 {
    let expires = Instant::now() + Duration::from_millis(300);
    let mut child = Owned(
        Command::new("/usr/bin/true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            return pid;
        }
        assert!(Instant::now() < expires);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn entry(detail: &str) -> String {
    format!(
        "{}\n",
        crate::journal_codec::entry(
            &EventArgs {
                detail: detail.into(),
                ..EventArgs::default()
            },
            Some(1),
            260
        )
    )
}
#[test]
fn abandoned_legacy_batches_keep_their_order_and_survive_pending_ring_pruning() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let owner = dead_owner();
    let first = path.join(format!("missed-notifications.held.{owner}.0"));
    fs::write(&first, entry("abandoned")).unwrap();
    fs::write(path.join("missed-notifications"), entry("pending")).unwrap();
    fs::write(path.join(format!("last-present.claim.{owner}.1")), "7\n").unwrap();
    fs::write(path.join("last-present"), "8\n").unwrap();
    let store = SqliteStore::new(path.clone());
    assert!(store.import_legacy().unwrap().is_empty());
    let _open = store.connect().unwrap();
    for n in 0..26 {
        store
            .record_journal(
                &EventArgs {
                    detail: format!("new {n}"),
                    ..EventArgs::default()
                },
                Some(n + 9),
                None,
            )
            .unwrap();
    }
    assert_eq!(
        crate::journal_codec::entries(&Journal::read(&store).unwrap().unwrap()).len(),
        25
    );
    let claimed = store.claim_return(Some(40), true).unwrap().unwrap();
    assert_eq!(claimed.since, Some(8));
    assert_eq!(claimed.waiting.len(), 1);
    assert_eq!(claimed.waiting[0].detail, "abandoned");
    assert_eq!(
        crate::journal_codec::entries(&Journal::read(&store).unwrap().unwrap())[0].detail,
        "new 1"
    );
    assert_eq!(fs::read(&first).unwrap(), entry("abandoned").as_bytes());
    store.complete_return().unwrap();
    store.import_legacy().unwrap();
    let later = store.claim_return(Some(41), true).unwrap().unwrap();
    assert_eq!(
        later.waiting.len(),
        25,
        "later arrivals retain their separate batch"
    );
    assert_eq!(later.waiting[0].detail, "new 1");
    store.complete_return().unwrap();
    assert!(
        store
            .claim_return(Some(41), true)
            .unwrap()
            .unwrap()
            .waiting
            .is_empty()
    );
}
#[test]
fn an_interrupted_import_leaves_no_completion_marker_or_partial_history() {
    const KEY: &str = "PNS_SQLITE_IMPORT_CHILD";
    let expires = Instant::now() + Duration::from_millis(700);
    if let Some(path) = std::env::var_os(KEY) {
        let path = std::path::PathBuf::from(path);
        let store = SqliteStore::new(path.clone());
        store
            .transaction(|transaction| {
                super::super::import::import(transaction, &path)?;
                fs::write(path.join("ready"), "imported but uncommitted")?;
                let until = Instant::now() + Duration::from_millis(350);
                while Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err::<(), _>(rusqlite::Error::InvalidQuery.into())
            })
            .unwrap_err();
        return;
    }
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("decisions"), "prior decision\n").unwrap();
    let store = SqliteStore::new(path.clone());
    let connection = store.connect().unwrap();
    let mut child = Owned(Command::new(std::env::current_exe().unwrap()).args(["--exact", "persistence::sqlite::tests::import_claims::an_interrupted_import_leaves_no_completion_marker_or_partial_history"]).env(KEY, &path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn().unwrap());
    while !path.join("ready").exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < expires);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(child);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM legacy_imports", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(pns_application::DecisionRing::read(&store).unwrap(), None);
    store.import_legacy().unwrap();
    assert_eq!(
        pns_application::DecisionRing::read(&store)
            .unwrap()
            .as_deref(),
        Some("prior decision\n")
    );
}

#[test]
fn legacy_ring_locks_defer_import_through_the_exact_stale_edge_and_unknown_or_future_clocks() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let lock = path.join("decisions.lock");
    fs::write(&lock, "").unwrap();
    let file = fs::File::options().write(true).open(&lock).unwrap();
    file.set_times(
        fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + Duration::from_secs(95)),
    )
    .unwrap();
    for now in [None, Some(94), Some(99), Some(100)] {
        assert!(
            super::super::import::inspect(&path, now).is_err(),
            "clock {now:?}"
        );
    }
    assert!(super::super::import::inspect(&path, Some(101)).is_ok());
    assert!(
        lock.is_file(),
        "import never removes a legacy ownership file"
    );
}
#[test]
fn an_unreadable_abandoned_window_is_reported_and_left_available_for_recovery() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let owner = dead_owner();
    let window = path.join(format!("last-present.claim.{owner}.1"));
    fs::create_dir(&window).unwrap();
    let store = SqliteStore::new(path);
    let failures = store.import_legacy().unwrap();
    assert_eq!(
        failures
            .iter()
            .map(|failure| failure.record.as_str())
            .collect::<Vec<_>>(),
        ["last-present"]
    );
    assert_eq!(store.last_present().unwrap(), None);
    assert!(window.is_dir());
}
