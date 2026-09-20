use super::*;

#[test]
fn starting_twice_and_reading_twice_takes_each_reading_once() {
    // PRESERVATION (C6): the concurrent path answers no differently from
    // the sequential one it replaces, however many times `start` and the
    // reads race each other. `forward_to_moshi` then `run_event` both
    // call `start` on the SAME probe set, and this is that shape: start,
    // read every probe, start again, read every probe again, every worker
    // joined by the time the last read returns.
    use pns_application::{ProbeStart, ScreenLockProbe, Wants};
    // The fixture directory is what the phone chain's terminal name
    // resolves against: a CI runner has no `/dev/ttys000`, so a real
    // device would make this assertion flaky by host rather than by
    // behavior.
    const JOINED_PHONE_ATIME: u64 = 1_650_000_000;
    let tty_dir = std::env::temp_dir().join(format!("pns-tty-join-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tty_dir);
    std::fs::create_dir_all(&tty_dir).expect("fixture dir");
    terminal_with_atime(&tty_dir, DISCOVERY_TERMINAL, JOINED_PHONE_ATIME);
    let registry = FakeRegistry::answering(5_000_000_000, true);
    let idle_reads = Arc::clone(&registry.idle_reads);
    let lock_reads = Arc::clone(&registry.lock_reads);
    let table = FakeTable::naming(&[DISCOVERY_TERMINAL]);
    let asked = Arc::clone(&table.asked);
    let probes = phone_probe(&DISCOVERY, Arc::new(table))
        .with_registry(Arc::new(registry))
        .with_tty_dir(tty_dir.to_string_lossy().into_owned());
    for _ in 0..2 {
        probes.start(Wants {
            desk: true,
            phone: true,
        });
        probes.idle_secs();
        probes.screen_locked();
        probes.phone_input_atime_secs();
    }
    assert_eq!(
        *probes.runner.calls.lock().unwrap(),
        vec!["/usr/bin/pgrep -x mosh-server".to_string()],
        "one spawn in the whole run"
    );
    assert_eq!(idle_reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(lock_reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(asked.lock().unwrap().len(), 1);
    // sol review, ROW 2: the counts above only pin the work; a
    // `join_desk`/`join_phone` that stored `None` for a successful worker
    // passed every one of them. Assert the joined values too.
    assert_eq!(probes.idle_secs(), Some(5), "the joined idle answer");
    assert_eq!(probes.screen_locked(), Some(true), "the joined lock answer");
    let phone_reading = probes.phone_input_atime_secs();
    let _ = std::fs::remove_dir_all(&tty_dir);
    assert_eq!(
        phone_reading,
        Some(JOINED_PHONE_ATIME),
        "the joined phone answer"
    );
}

#[test]
fn a_lock_probe_already_answered_inline_is_not_retaken_by_a_later_start() {
    // sol review, ROW 1: `start` gated the desk spawn on the idle cell
    // only, so a caller reading the lock BEFORE starting the desk pair
    // took the lock reading a second time on the spawned thread, and
    // `join_desk`'s `OnceCell::set` silently dropped that second answer.
    // No production caller reads in this order, but nothing enforced it.
    use pns_application::{IdleProbe, ProbeStart, ScreenLockProbe, Wants};
    let registry = FakeRegistry::answering(5_000_000_000, true);
    let lock_reads = Arc::clone(&registry.lock_reads);
    let probes = desk_probes(registry);
    assert_eq!(probes.screen_locked(), Some(true));
    probes.start(Wants {
        desk: true,
        phone: false,
    });
    assert_eq!(probes.idle_secs(), Some(5));
    assert_eq!(
        lock_reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a lock answer already taken inline must not be retaken"
    );
}
