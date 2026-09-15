use std::time::Duration;

pub(super) fn wait_grace(
    grace: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut pause: impl FnMut(Duration),
) {
    loop {
        let remaining = grace.saturating_sub(elapsed());
        if remaining.is_zero() {
            break;
        }
        pause(remaining.min(Duration::from_millis(250)));
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
            |duration| {
                assert!(elapsed.get() < Duration::from_secs(2));
                assert!(duration <= Duration::from_millis(250));
                elapsed.set(elapsed.get() + duration);
                polls.set(polls.get() + 1);
            },
        );
        assert_eq!(elapsed.get(), Duration::from_secs(2));
        assert_eq!(polls.get(), 8);
    }
}
