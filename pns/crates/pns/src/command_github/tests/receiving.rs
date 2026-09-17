mod tests {
    use super::super::github_test_fixture::{CURSOR, NOW, a_prior_poll, thread};
    use super::super::*;
    use crate::runtime_test_support::scratch;
    use pns_adapters::GithubPolled;

    /// One answer carrying one thread, as both transports' polls see it.
    fn listed() -> GithubPolled {
        GithubPolled::Listed {
            threads: vec![thread("webdavis/dotfiles", "4471")],
            answer: Answer {
                identities: Vec::new(),
                last_modified: CURSOR.to_string(),
                interval_secs: Some(60),
            },
        }
    }

    #[test]
    fn a_pushed_item_and_the_next_polled_one_are_delivered_once_between_them() {
        // THE PUSH IS A DOORBELL AND NOT A SECOND SOURCE: the poll it rings
        // and the scheduled tick after it read one state file, so the second
        // of the two finds the identity already reported. THE MUTANT THIS
        // PINS: a receiver that submits the payload itself, which would spell
        // its own identity and deliver every event twice.
        let state = scratch("push-then-poll");
        let mut submitted: Vec<String> = Vec::new();
        // The doorbell's poll, on an established source.
        report(
            &a_prior_poll(),
            &state,
            listed(),
            NOW,
            Launch::Daemon,
            &mut |event| submitted.push(event.identity.clone()),
        );
        // The scheduled tick a minute later, reading what that published.
        report(
            &pns_adapters::read_poll_state(&state),
            &state,
            listed(),
            NOW + 60,
            Launch::Daemon,
            &mut |event| submitted.push(event.identity.clone()),
        );
        assert_eq!(
            submitted,
            vec!["webdavis/dotfiles|workflow_run|4471|1789398987"],
            "one notification reached the channel more than once"
        );
    }

    #[test]
    fn a_poll_that_finds_the_lock_held_stands_down_instead_of_racing_the_holder() {
        // THE MUTANT THIS PINS: two polls in two processes (the doorbell's and
        // the daemon's) reading the state before either published it, which
        // makes the same notification fresh to both. The lock is what makes
        // the two transports one path.
        let state = scratch("poll-lock");
        let lock = state.join(GITHUB_POLL_LOCK);
        assert!(pns_adapters::claim_lock(
            &lock,
            NOW,
            GITHUB_POLL_LOCK_STALE_SECS
        ));
        assert!(
            !pns_adapters::claim_lock(&lock, NOW, GITHUB_POLL_LOCK_STALE_SECS),
            "a second poll took a lock the first one holds"
        );
    }
}
