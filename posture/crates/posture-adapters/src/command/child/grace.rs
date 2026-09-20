use std::time::Duration;

/// How often the wait below asks whether the child has gone.
const POLL: Duration = Duration::from_millis(1);

/// Wait for the signalled child to go, for at most `grace`.
///
/// The duration bounds a child that ignores SIGTERM; one that handles it is
/// waited for and no longer, so the kill that follows cannot land on a
/// handler still writing its last bytes.
pub(super) fn wait_grace(
    grace: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut exited: impl FnMut() -> bool,
    mut pause: impl FnMut(Duration),
) {
    loop {
        if exited() {
            break;
        }
        let remaining = grace.saturating_sub(elapsed());
        if remaining.is_zero() {
            break;
        }
        pause(remaining.min(POLL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn production_grace_waits_two_seconds_before_allowing_kill() {
        let elapsed = Cell::new(Duration::ZERO);
        let polls = Cell::new(0);
        wait_grace(
            Duration::from_secs(2),
            || elapsed.get(),
            || false,
            |duration| {
                assert!(elapsed.get() < Duration::from_secs(2));
                assert!(duration <= POLL);
                elapsed.set(elapsed.get() + duration);
                polls.set(polls.get() + 1);
            },
        );
        assert_eq!(elapsed.get(), Duration::from_secs(2));
        assert_eq!(polls.get(), 2_000);
    }

    #[test]
    fn a_child_that_goes_on_the_signal_is_never_waited_out() {
        let polls = Cell::new(0);
        wait_grace(
            Duration::from_secs(2),
            || Duration::ZERO,
            || true,
            |_| polls.set(polls.get() + 1),
        );
        assert_eq!(polls.get(), 0);
    }
}
