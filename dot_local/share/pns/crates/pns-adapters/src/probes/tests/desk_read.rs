use super::*;

#[test]
fn the_idle_probe_reports_whole_seconds_from_the_nanosecond_count() {
    let probes = probes_answering("\"HIDIdleTime\" = 5000000000\n");
    assert_eq!(probes.idle_secs(), Some(5));
}

#[test]
fn an_idle_command_that_fails_reports_unknown_which_fails_open_into_a_push() {
    assert_eq!(probes_failing().idle_secs(), None);
}

#[test]
fn a_garbled_idle_count_is_unknown_rather_than_zero_seconds_idle() {
    // Zero would read as "actively typing" and silently drop the push.
    let probes = probes_answering("\"HIDIdleTime\" = not-a-number\n");
    assert_eq!(probes.idle_secs(), None);
}

#[test]
fn the_idle_probe_argv_matches_the_bash_original() {
    let probes = probes_answering("\"HIDIdleTime\" = 5000000000\n");
    probes.idle_secs();
    assert_eq!(
        probes.runner.calls.lock().unwrap()[0],
        "/usr/sbin/ioreg -c IOHIDSystem"
    );
}

#[test]
fn the_lock_probe_reads_the_root_dictionary_by_exact_argv_and_only_once() {
    // The Root node with its own properties, which is where the console
    // aggregate is printed; `-d1` keeps it to that one node. Once per
    // invocation, like every reading here: the blocked path asks where
    // the operator is twice by design and both answers must be the same
    // measurement.
    use pns_application::ScreenLockProbe;
    let probes = SystemProbes::new(
        ExactArgvRunner {
            answers: vec![(
                "/usr/sbin/ioreg -n Root -d1".to_string(),
                ROOT_LOCKED.to_string(),
            )],
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    );
    assert_eq!(probes.screen_locked(), Some(true));
    assert_eq!(
        probes.screen_locked(),
        Some(true),
        "and the same answer both times"
    );
    assert_eq!(
        *probes.runner.calls.lock().unwrap(),
        vec!["/usr/sbin/ioreg -n Root -d1".to_string()],
        "the exact argv, taken once"
    );
}

#[test]
fn the_lock_is_not_spawned_where_idle_failed() {
    // The desk thread's own body only reads the lock where idle parsed
    // (`start`'s doc), so a failed idle must leave the lock cell filled
    // from that SAME join rather than answered by a second `ioreg`
    // spawn when `screen_locked` is read afterward: the trait-level
    // `lock_reads == 0` assertion beside this one counts calls to the
    // METHOD, never spawns, and this is the one that counts the spawn.
    use pns_application::{IdleProbe, ProbeStart, ScreenLockProbe, Wants};
    let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let probes = SystemProbes::new(
        CountingRunner {
            answer: "nothing the parser recognizes".to_string(),
            calls: Arc::clone(&calls),
        },
        "/nonexistent/marker".to_string(),
    );
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
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one ioreg spawn total: the desk thread never asked for the lock, \
         and the later read found the cell the join already filled"
    );
}

#[test]
fn a_slow_probe_does_not_hold_up_a_fast_one() {
    // PROVEN BY ORDER, NEVER BY TIME (C4). Concurrent: the phone
    // thread's `pgrep` releases the desk thread's blocked `ioreg`
    // within microseconds, so idle reads its fixture value and this
    // test returns at once. A sequential, desk-only, or
    // join-at-start mutant never starts the phone thread before
    // blocking on idle, so `ioreg` times out at 2 s into no reading
    // at all, and the assertion below is what turns that into red.
    use pns_application::{IdleProbe, PhoneInputProbe, ProbeStart, ScreenLockProbe, Wants};
    let (release, wait) = std::sync::mpsc::channel();
    let probes = SystemProbes::new(
        GateRunner {
            release,
            wait: Mutex::new(wait),
            idle_answer: "\"HIDIdleTime\" = 5000000000".to_string(),
        },
        "/marker".to_string(),
    );
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
        "the phone thread's own pgrep released the blocked ioreg"
    );
}
