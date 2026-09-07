use super::*;

// --- the tier the marker decides --------------------------------------------

/// A hue config pointed at a listener nobody should reach unless the turn
/// earned a pulse. The signal is silent, so the CONNECTION is the
/// observation; the socket is closed the instant it arrives, because a
/// listener that accepts and says nothing makes the client wait out its own
/// deadline instead.
fn hue_listener(sandbox: &Sandbox) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    let reached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&reached);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            drop(stream);
        }
    });
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        format!(
            "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             [plugins.hermes]\nenabled = true\n"
        ),
    )
    .expect("config");
    reached
}

/// How many times the bridge was reached, after a settle for the connection
/// to land.
fn bridge_calls(reached: &std::sync::atomic::AtomicUsize) -> usize {
    std::thread::sleep(std::time::Duration::from_millis(100));
    reached.load(std::sync::atomic::Ordering::SeqCst)
}

#[test]
fn a_turn_long_enough_pulses_and_a_short_one_does_not() {
    // The marker's elapsed time is the ONLY thing that differs between these
    // two runs, which is what makes it the wiring under test.
    for (label, started_secs_ago, expected) in
        [("a long turn", 9_000, true), ("a short turn", 5, false)]
    {
        let sandbox = Sandbox::new(&format!("hook-tier-{}", started_secs_ago));
        let reached = hue_listener(&sandbox);
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            - started_secs_ago;
        std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");
        let mut child = spawn_hook(with_state_dir(&sandbox), "stop");
        write_payload(
            &mut child,
            br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#,
        );
        assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
        assert_eq!(bridge_calls(&reached) > 0, expected, "{label}");
    }
}

#[test]
fn a_long_turn_that_died_still_earns_its_pulse() {
    // The tier does not care HOW the turn ended: the operator who walked away
    // from a long run is exactly the one the lights are for, and this is the
    // first time a hook can reach the red half of the pulse at all.
    //
    // THE LISTENER COUNTS CONNECTIONS AND NEVER READS THE BODY (it closes the
    // socket the instant it arrives), so this pins that the pulse fired, not
    // that it was red. The colour is decided by `event.state`, which the
    // failed-turn test above pins as `failed`.
    let sandbox = Sandbox::new("hook-stop-failure-tier");
    let reached = hue_listener(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        - 9_000;
    std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");
    let mut child = spawn_hook(with_state_dir(&sandbox), "stop-failure");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","error":"API Error: 500"}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert!(
        bridge_calls(&reached) > 0,
        "a turn that earned the tier still earns it when it dies"
    );
}

#[test]
fn two_stops_racing_one_turn_cannot_both_report_it_long() {
    // The claim is a rename, so exactly one of them can win it.
    let sandbox = Sandbox::new("hook-stop-race");
    let reached = hue_listener(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        - 9_000;
    std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");

    let payload = br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#;
    // BOTH start before either is fed, so they are genuinely in flight
    // together rather than one finishing while the next is still spawning.
    let mut children: Vec<_> = (0..2)
        .map(|_| spawn_hook(with_state_dir(&sandbox), "stop"))
        .collect();
    for child in &mut children {
        write_payload(child, payload);
    }
    for child in children.drain(..) {
        assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    }
    assert_eq!(
        bridge_calls(&reached),
        1,
        "exactly one Stop can claim the turn, so exactly one pulses"
    );
}
