use super::*;

#[test]
fn a_reading_asked_for_twice_is_still_taken_once() {
    // The blocked path asks where the operator is TWICE by design: once to
    // decide whether an approval is forwarded to the phone at all, and
    // again to decide what the notification delivers. Two spawns can
    // answer differently, and a freshness boundary crossed between them
    // cards a phone with no round trip behind it.
    //
    // INLINE, START-FREE, ON PURPOSE (C5): a started desk thread makes two
    // runner calls by design (idle, then the lock it qualifies), so this
    // stays a plain read to keep pinning "no start, no thread, one call".
    use pns_application::IdleProbe;
    let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let probes = SystemProbes::new(
        CountingRunner {
            answer: "\"HIDIdleTime\" = 5000000000".to_string(),
            calls: Arc::clone(&calls),
        },
        "/nonexistent/marker".to_string(),
    );
    assert_eq!(probes.idle_secs(), Some(5));
    assert_eq!(
        probes.idle_secs(),
        Some(5),
        "and the same answer both times"
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one probe set is one reading"
    );
}

#[test]
fn a_reading_that_came_back_empty_is_not_retaken_either() {
    // An unreadable probe is an ANSWER, and re-taking it would let two
    // consumers disagree about a machine that told the first one nothing.
    use pns_application::IdleProbe;
    let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let probes = SystemProbes::new(
        CountingRunner {
            answer: "nothing the parser recognizes".to_string(),
            calls: Arc::clone(&calls),
        },
        "/nonexistent/marker".to_string(),
    );
    assert_eq!(probes.idle_secs(), None);
    assert_eq!(probes.idle_secs(), None);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn the_clock_is_the_fifth_memoized_reading() {
    // A wall clock read twice can answer twice: a second boundary between
    // the two reads is exactly what drifted a phone reading and a desk
    // reading apart in R4-1. Seeding a fixed value and asking twice is
    // what proves this answers the CELL and not the clock: a mutant that
    // bypassed the cell would answer the real epoch here, not 42.
    let probes = probes_answering("unused").with_clock(42);
    assert_eq!(probes.now_secs(), Some(42));
    assert_eq!(
        probes.now_secs(),
        Some(42),
        "and the same answer both times"
    );
}
