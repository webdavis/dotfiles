use std::time::Duration;

/// Poll a child to exit, up to a deadline. There is no wait-with-timeout in
/// the standard library and macOS ships no `timeout(1)`.
///
/// THE SLEEPER IS A PARAMETER because the schedule is only worth anything at
/// the line that sleeps it. `next_poll_interval` can compute the whole backoff
/// correctly while this loop sleeps a flat ceiling, which is the shape the
/// latency fix removed, so a test watches the durations this hands its sleeper
/// rather than a clock.
pub(super) fn wait_until(
    child: &mut std::process::Child,
    expires_at: std::time::Instant,
    mut sleep: impl FnMut(Duration),
) -> Option<std::process::ExitStatus> {
    let mut interval = FIRST_POLL_INTERVAL;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Err(_) => return None,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= expires_at {
            return None;
        }
        sleep(interval);
        interval = next_poll_interval(interval);
    }
}

/// The LONGEST a bounded wait sleeps between checks. Long enough not to spin a
/// core while a genuinely wedged child runs out its deadline.
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// And the SHORTEST, which is where every wait starts.
///
/// THE CEILING USED TO BE THE ONLY INTERVAL, and it was charged to every
/// bounded spawn. This wait begins after the child's stdout has already hit
/// EOF, which is a child on its way out, so the check that matters is the one
/// taken microseconds later and the ceiling is what a run pays for missing it.
pub(super) const FIRST_POLL_INTERVAL: Duration = Duration::from_micros(200);

/// The next gap between checks: doubled, and never past the ceiling. The
/// backoff is what keeps the fast start from becoming thousands of wakeups a
/// second on the one path where a child really is wedged.
pub(super) fn next_poll_interval(current: Duration) -> Duration {
    current.saturating_mul(2).min(POLL_INTERVAL)
}

#[cfg(test)]
mod tests;
