//! Every expectation was read off the alerter this replaces
//! (`executable_results-alerter.sh`, `_checkpoint` and the lock block).

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "posture-cursor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root.join("offset")
}

#[test]
fn a_cursor_survives_the_round_trip_it_is_written_for() {
    let path = scratch();
    let store = CursorFile::new(path.clone());
    assert!(store.read().is_none(), "nothing recorded yet");
    store.write(StoredCursor {
        inode: 12_345,
        offset: 67_890,
    });
    assert_eq!(
        store.read(),
        Some(StoredCursor {
            inode: 12_345,
            offset: 67_890
        })
    );
}

#[test]
fn a_cursor_is_published_by_rename_and_leaves_no_pending_file_behind() {
    // A run killed mid-write must leave the OLD position intact rather than a
    // half-written one, which the domain would refuse and replay over.
    let path = scratch();
    CursorFile::new(path.clone()).write(StoredCursor {
        inode: 7,
        offset: 250,
    });
    let strays: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "offset")
        .collect();
    assert!(strays.is_empty(), "{strays:?}");
}

#[test]
fn anything_that_is_not_a_cursor_reads_as_none_rather_than_as_a_guess() {
    // Guessing at a damaged cursor is how a wrong offset gets trusted and a
    // batch is skipped in silence. `None` replays the log and says so.
    for contents in ["", "\n", "7", "seven 250", "7 250 extra", "-1 250"] {
        let path = scratch();
        fs::write(&path, contents).unwrap();
        assert_eq!(CursorFile::new(path).read(), None, "{contents:?}");
    }
}

#[test]
fn a_cursor_written_twice_holds_only_the_second_answer() {
    let path = scratch();
    let store = CursorFile::new(path);
    store.write(StoredCursor {
        inode: 7,
        offset: 100,
    });
    store.write(StoredCursor {
        inode: 7,
        offset: 250,
    });
    assert_eq!(store.read().unwrap().offset, 250);
}

#[test]
fn one_run_takes_the_lock_and_a_second_over_the_same_file_does_not() {
    // A WatchPaths burst fires several invocations. Two runs would judge the
    // same rows, page twice and race each other's checkpoint.
    let cursor = scratch();
    let first = SingleRunLock::beside(&cursor);
    let second = SingleRunLock::beside(&cursor);
    assert!(first.taken());
    assert!(!second.taken(), "a contended run must be a clean no-op");
}

#[test]
fn asking_twice_in_one_run_is_still_held_rather_than_refused() {
    // The lock is asked about at the top of the transaction and must not
    // report itself contended by its own earlier answer.
    let lock = SingleRunLock::beside(&scratch());
    assert!(lock.taken());
    assert!(lock.taken());
}

#[test]
fn the_lock_is_released_when_its_holder_goes_away() {
    // The kernel releases it on ANY exit, so there is no stale-lock state to
    // clean up and no recovery path to get wrong.
    let cursor = scratch();
    assert!(SingleRunLock::beside(&cursor).taken());
    assert!(
        SingleRunLock::beside(&cursor).taken(),
        "the first lock's holder was dropped, so the file is free again"
    );
}

#[test]
fn a_lock_whose_directory_cannot_be_made_refuses_rather_than_running_unlocked() {
    // FAIL CLOSED. Running unlocked beside a sibling that holds the lock is the
    // double-delivery this exists to prevent.
    let blocked = std::env::temp_dir().join(format!("posture-lock-file-{}", std::process::id()));
    let _ = fs::remove_dir_all(&blocked);
    fs::write(&blocked, b"not a directory\n").unwrap();
    assert!(!SingleRunLock::beside(&blocked.join("offset")).taken());
    let _ = fs::remove_file(&blocked);
}
