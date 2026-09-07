use super::*;

/// How many steps of the schedule the test below watches: enough to pass
/// the ceiling, which is where a call site that doubles without capping
/// parts company with one that does not.
const WATCHED_STEPS: usize = 8;

#[test]
fn the_wait_between_checks_doubles_and_stops_at_the_ceiling() {
    // The ceiling is what keeps a wedged child from being polled thousands
    // of times a second for the whole deadline; the doubling is what keeps
    // the ordinary case, a child already exiting, off a flat 10ms bill.
    assert_eq!(
        next_poll_interval(FIRST_POLL_INTERVAL),
        FIRST_POLL_INTERVAL * 2
    );
    // AND THE FIRST INTERVAL HAS TO GROW, which doubling alone does not
    // give: zero doubles to zero, so a first interval of nothing is a
    // `try_wait` spin for the whole deadline rather than the backoff the
    // ceiling above was written to guarantee.
    assert!(
        next_poll_interval(FIRST_POLL_INTERVAL) > FIRST_POLL_INTERVAL,
        "a wait that starts at zero never leaves it"
    );
    assert_eq!(next_poll_interval(POLL_INTERVAL / 2), POLL_INTERVAL);
    assert_eq!(next_poll_interval(POLL_INTERVAL), POLL_INTERVAL);
    assert!(FIRST_POLL_INTERVAL < POLL_INTERVAL, "it starts below it");
}

#[test]
fn the_wait_sleeps_the_schedule_it_computes_rather_than_a_flat_ceiling() {
    // THE HELPER ABOVE IS NOT THE FIX. A correct backoff computed beside a
    // loop that still sleeps `POLL_INTERVAL` leaves every assertion up
    // there green and every bounded spawn paying the flat bill again, so
    // what is pinned here is the LINE THAT SLEEPS: the durations the loop
    // hands its sleeper, in order, against the schedule the helper states.
    //
    // NOTHING SLEEPS AND NOTHING IS TIMED. The fake sleeper returns at
    // once, so the child is alive for exactly as many polls as it allows
    // and dropping its stdin is what ends the wait. `cat` holds that pipe
    // open until it does.
    let mut child = std::process::Command::new("/bin/cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("a child to wait on");
    let mut stdin = child.stdin.take();
    let mut slept: Vec<Duration> = Vec::new();
    wait_until(
        &mut child,
        std::time::Instant::now() + Duration::from_secs(5),
        |interval| {
            slept.push(interval);
            if slept.len() >= WATCHED_STEPS {
                drop(stdin.take());
            }
        },
    );
    // DERIVED, so the expectation cannot drift away from the constants: it
    // is the schedule `next_poll_interval` defines, walked from the first
    // interval. What the test states is that the loop walks it too.
    let schedule: Vec<Duration> = std::iter::successors(Some(FIRST_POLL_INTERVAL), |current| {
        Some(next_poll_interval(*current))
    })
    .take(WATCHED_STEPS)
    .collect();
    assert!(
        slept.len() >= WATCHED_STEPS,
        "the wait polled {} times, too few to show a schedule",
        slept.len()
    );
    assert_eq!(
        &slept[..WATCHED_STEPS],
        schedule.as_slice(),
        "the loop slept its own schedule"
    );
}
