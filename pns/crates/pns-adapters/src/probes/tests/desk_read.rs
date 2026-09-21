use super::*;

#[test]
fn the_idle_probe_reports_whole_seconds_from_the_nanosecond_count() {
    let probes = desk_probes(FakeRegistry::answering(5_000_000_000, false));
    assert_eq!(probes.idle_secs(), Some(5));
}

#[test]
fn an_idle_property_that_cannot_be_read_reports_unknown_which_fails_open_into_a_push() {
    // THE UNREADABLE DEVICE, injected: a registry that refuses the property
    // must leave the reading unknown rather than coerce to 0, which would
    // read as "actively typing" and silently drop the push.
    assert_eq!(probes_failing().idle_secs(), None);
}

#[test]
fn the_lock_reading_follows_the_console_through_a_lock_and_an_unlock() {
    // BOTH SIDES OF THE TRANSITION, which the live machine could only show
    // one of at a time: the aggregate is reported as it is read, and the
    // fail direction below is what an unreadable one gets instead.
    use pns_application::ScreenLockProbe;
    let locked = desk_probes(FakeRegistry::answering(5_000_000_000, true));
    let unlocked = desk_probes(FakeRegistry::answering(5_000_000_000, false));
    assert_eq!(locked.screen_locked(), Some(true));
    assert_eq!(unlocked.screen_locked(), Some(false));
}

#[test]
fn a_lock_property_that_cannot_be_read_is_unknown_rather_than_unlocked() {
    // THE FAIL DIRECTION: only `Some(true)` locks, so unknown leaves the
    // shipped desk-freshness behavior in place instead of killing the desk
    // banner wherever this property is renamed or dropped.
    use pns_application::ScreenLockProbe;
    assert_eq!(probes_failing().screen_locked(), None);
}

#[test]
fn the_lock_property_is_read_once_however_often_it_is_asked_for() {
    // Once per invocation, like every reading here: the blocked path asks
    // where the operator is twice by design and both answers must be the
    // same measurement.
    use pns_application::ScreenLockProbe;
    let registry = FakeRegistry::answering(5_000_000_000, true);
    let reads = Arc::clone(&registry.lock_reads);
    let probes = desk_probes(registry);
    assert_eq!(probes.screen_locked(), Some(true));
    assert_eq!(
        probes.screen_locked(),
        Some(true),
        "and the same answer both times"
    );
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "taken once"
    );
}

#[test]
fn the_lock_is_not_read_at_all_where_idle_failed() {
    // The desk thread's own body only reads the lock where idle answered
    // (`start`'s doc), so a failed idle must leave the lock cell filled
    // from that SAME join rather than answered by a second registry read
    // when `screen_locked` is asked for afterward.
    use pns_application::{IdleProbe, ProbeStart, ScreenLockProbe, Wants};
    let registry = FakeRegistry::unreadable();
    let lock_reads = Arc::clone(&registry.lock_reads);
    let probes = desk_probes(registry);
    probes.start(Wants {
        desk: true,
        phone: false,
    });
    assert_eq!(probes.idle_secs(), None);
    assert_eq!(
        probes.screen_locked(),
        None,
        "never attempted, not merely unreadable"
    );
    assert_eq!(
        lock_reads.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the desk thread never asked for the lock, and the later read \
         found the cell the join already filled"
    );
}

#[test]
fn a_slow_probe_does_not_hold_up_a_fast_one() {
    // PROVEN BY ORDER, NEVER BY TIME (C4). Concurrent: the phone thread's
    // `pgrep` releases the desk thread's blocked registry read within
    // microseconds, so idle reads its fixture value and this test returns
    // at once. A sequential, desk-only, or join-at-start mutant never
    // starts the phone thread before blocking on idle, so the registry
    // read times out at 2 s into no reading at all.
    use pns_application::{IdleProbe, PhoneInputProbe, ProbeStart, ScreenLockProbe, Wants};
    let (release, wait) = std::sync::mpsc::channel();
    let probes = SystemProbes::new(GateRunner { release }, "/marker".to_string())
        .with_registry(Arc::new(GateRegistry {
            wait: Mutex::new(wait),
            idle_nanoseconds: 5_000_000_000,
        }))
        .with_table(Arc::new(FakeTable::naming(&[])));
    probes.start(Wants {
        desk: true,
        phone: true,
    });
    // PRODUCTION ORDER: idle, then the lock it qualifies, then phone.
    let idle = probes.idle_secs();
    let _ = probes.screen_locked();
    let _ = probes.phone_input_atime_secs();
    assert_eq!(
        idle,
        Some(5),
        "the phone thread's own pgrep released the blocked registry read"
    );
}

// --- the deadline, which a native call keeps through a channel ----------

#[test]
fn a_registry_read_that_never_returns_leaves_the_reading_unknown() {
    // THE STALLED NATIVE CALL. The thread behind it cannot be killed, so
    // it is left behind; what must not happen is the notification path
    // waiting on it. The deadline is named here so the proof costs
    // milliseconds rather than the production window.
    let stalled: Arc<dyn ConsoleRegistry> = Arc::new(StalledRegistry);
    let deadline = Duration::from_millis(20);
    assert_eq!(idle_reading_within(&stalled, deadline), None);
    assert_eq!(lock_reading_within(&stalled, deadline), None);
}
