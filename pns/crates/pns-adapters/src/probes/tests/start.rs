use super::*;

#[test]
fn starting_twice_and_reading_twice_spawns_each_probe_once() {
    // PRESERVATION (C6): the concurrent path answers no differently from
    // the sequential one it replaces, however many times `start` and the
    // reads race each other. `forward_to_moshi` then `run_event` both
    // call `start` on the SAME probe set (main.rs:1939, :2383), and this
    // is that shape: start, read every probe, start again, read every
    // probe again, every worker joined by the time the last read
    // returns.
    use pns_application::{ProbeStart, ScreenLockProbe, Wants};
    let mut scripted: Vec<(String, String)> = DISCOVERY
        .iter()
        .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
        .collect();
    scripted.push((
        "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
        "\"HIDIdleTime\" = 5000000000".to_string(),
    ));
    scripted.push((
        "/usr/sbin/ioreg -n Root -d1".to_string(),
        ROOT_LOCKED.to_string(),
    ));
    // DISCOVERY's canned `ps` output names "ttys000", a real `/dev`
    // entry only on a machine with an open terminal by that name. A CI
    // runner has none, so the phone join reads None there against
    // TTY_DIR and this test's own "joined value" assertion goes flaky
    // by host rather than by behavior. The fixture directory below is
    // what `with_tty_dir` points the phone chain at instead, so the
    // assertion is a real number every machine can produce.
    const JOINED_PHONE_ATIME: u64 = 1_650_000_000;
    let tty_dir = std::env::temp_dir().join(format!("pns-tty-join-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tty_dir);
    std::fs::create_dir_all(&tty_dir).expect("fixture dir");
    terminal_with_atime(&tty_dir, "ttys000", JOINED_PHONE_ATIME);
    let probes = SystemProbes::new(
        ExactArgvRunner {
            answers: scripted,
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    )
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
    let calls = probes.runner.calls.lock().unwrap();
    for expected in [
        "/usr/sbin/ioreg -c IOHIDSystem",
        "/usr/sbin/ioreg -n Root -d1",
        "/usr/bin/pgrep -x mosh-server",
        "/usr/bin/pgrep -P 14362",
        "/bin/ps -o tty= -p 14363",
    ] {
        assert_eq!(
            calls.iter().filter(|call| *call == expected).count(),
            1,
            "case: {expected}, calls were {calls:?}"
        );
    }
    drop(calls);
    // sol review, ROW 2: the assertions above only count runner calls;
    // a `join_desk`/`join_phone` that stored `None` for a successful
    // worker passed every one of them. Assert the joined values too.
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
    // ran `ioreg -n Root -d1` a second time on the spawned thread, and
    // `join_desk`'s `OnceCell::set` silently dropped that second answer.
    // No production caller reads in this order, but nothing enforced it.
    use pns_application::{ProbeStart, ScreenLockProbe, Wants};
    let probes = SystemProbes::new(
        ExactArgvRunner {
            answers: vec![
                (
                    "/usr/sbin/ioreg -n Root -d1".to_string(),
                    ROOT_LOCKED.to_string(),
                ),
                (
                    "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
                    "\"HIDIdleTime\" = 5000000000".to_string(),
                ),
            ],
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    );
    assert_eq!(probes.screen_locked(), Some(true));
    probes.start(Wants {
        desk: true,
        phone: false,
    });
    assert_eq!(probes.idle_secs(), Some(5));
    let calls = probes.runner.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|call| *call == "/usr/sbin/ioreg -n Root -d1")
            .count(),
        1,
        "a lock answer already taken inline must not be retaken: {calls:?}"
    );
}
