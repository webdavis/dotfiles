mod tests {
    use super::super::github_test_fixture::{CURSOR, NOW, a_prior_poll, thread};
    use super::super::*;
    use crate::runtime_test_support::scratch;
    use pns_adapters::GithubPolled;
    use pns_domain::github::poll::Seen;

    #[test]
    fn the_poll_argument_parser_tells_the_daemons_launch_from_a_typed_one() {
        // THE FLAG IS THE DAEMON'S OWN SPELLING, and it is what the daemon
        // registers, so the two are read by one parser rather than guessed at
        // from the environment. An unknown word is a refusal, never a silent
        // fallthrough into a poll the operator believes ran differently.
        assert_eq!(github_launch(&[]), Some(Launch::Operator));
        assert_eq!(
            github_launch(&[GITHUB_DAEMON_FLAG.to_string()]),
            Some(Launch::Daemon)
        );
        for arguments in [
            vec!["--dameon".to_string()],
            vec!["--daemon=1".to_string()],
            vec![String::new()],
            vec![
                GITHUB_DAEMON_FLAG.to_string(),
                GITHUB_DAEMON_FLAG.to_string(),
            ],
        ] {
            assert_eq!(github_launch(&arguments), None, "{arguments:?} was read");
        }
    }

    // --- what one answer does to the durable state -----------------------

    #[test]
    fn a_304_that_changes_nothing_publishes_nothing_at_all() {
        // THE MUTANT THIS PINS: a write on every tick, which is a rename in
        // the state directory once a minute forever for no reason. The quiet
        // case is the common case.
        let state = scratch("304-quiet");
        let stored = PollState {
            last_modified: CURSOR.to_string(),
            interval_secs: 60,
            seen: Vec::new(),
        };
        assert_eq!(
            report(
                &stored,
                &state,
                GithubPolled::NotModified {
                    interval_secs: Some(60)
                },
                NOW,
                Launch::Daemon,
                &mut |_| panic!("a 304 submitted something")
            ),
            0
        );
        assert!(
            !state.join(pns_adapters::GITHUB_POLL_STATE).exists(),
            "a 304 that moved nothing still wrote the file"
        );
    }

    #[test]
    fn a_304_raising_the_interval_publishes_it_and_keeps_the_cursor() {
        // "In times of high server load, the time may increase. Please obey
        // the header." Obeying it means the new interval is durable, and the
        // cursor a 304 never restates is kept rather than cleared.
        let state = scratch("304-interval");
        let stored = PollState {
            last_modified: CURSOR.to_string(),
            interval_secs: 60,
            seen: vec![Seen {
                identity: "kept".to_string(),
                first_seen: NOW,
            }],
        };
        report(
            &stored,
            &state,
            GithubPolled::NotModified {
                interval_secs: Some(300),
            },
            NOW,
            Launch::Daemon,
            &mut |_| panic!("a 304 submitted something"),
        );
        let published = pns_adapters::read_poll_state(&state);
        assert_eq!(published.interval_secs, 300);
        assert_eq!(published.last_modified, CURSOR, "the cursor was cleared");
        assert_eq!(published.seen.len(), 1, "the seen-set was cleared");
    }

    #[test]
    fn a_401_exits_non_zero_and_never_looks_like_an_empty_listing() {
        // THE MUTANT THIS PINS: a refusal reported as nothing to do. A
        // revoked token would leave a source that looks alive and reports
        // nothing, with no reading to go stale in its place.
        let state = scratch("401");
        for launch in [Launch::Daemon, Launch::Operator] {
            assert_eq!(
                report(
                    &PollState::default(),
                    &state,
                    GithubPolled::Unauthorized { status: 401 },
                    NOW,
                    launch,
                    &mut |_| panic!("a refusal submitted something")
                ),
                1,
                "{launch:?}"
            );
        }
        assert!(
            !state.join(pns_adapters::GITHUB_POLL_STATE).exists(),
            "a refusal advanced the cursor"
        );
    }

    #[test]
    fn a_rate_limit_and_a_5xx_are_neither_of_them_the_configuration_refusal() {
        // A RATE LIMIT IS NOT A TOKEN PROBLEM: it exits non-zero, because
        // this poll listed nothing, and says nothing about the vault. A 5xx
        // is transient and exits zero, so launchd and the daemon read it as
        // an ordinary tick.
        let state = scratch("transient");
        assert_eq!(
            report(
                &PollState::default(),
                &state,
                GithubPolled::RateLimited,
                NOW,
                Launch::Operator,
                &mut |_| panic!("a rate limit submitted something")
            ),
            1
        );
        assert_eq!(
            report(
                &PollState::default(),
                &state,
                GithubPolled::Unavailable {
                    detail: "connection refused".to_string()
                },
                NOW,
                Launch::Daemon,
                &mut |_| panic!("a 304 submitted something")
            ),
            0
        );
        assert!(!state.join(pns_adapters::GITHUB_POLL_STATE).exists());
    }

    #[test]
    fn a_listing_publishes_the_cursor_and_remembers_what_it_reported() {
        let state = scratch("listed");
        let threads = vec![thread("webdavis/dotfiles", "4471")];
        let mut submitted: Vec<String> = Vec::new();
        report(
            &a_prior_poll(),
            &state,
            GithubPolled::Listed {
                threads,
                answer: Answer {
                    identities: Vec::new(),
                    last_modified: CURSOR.to_string(),
                    interval_secs: Some(60),
                },
            },
            NOW,
            Launch::Daemon,
            &mut |event| submitted.push(event.identity.clone()),
        );
        assert_eq!(
            submitted,
            vec!["webdavis/dotfiles|workflow_run|4471|1789398987"]
        );
        let published = pns_adapters::read_poll_state(&state);
        assert_eq!(published.last_modified, CURSOR);
        assert_eq!(published.interval_secs, 60);
        assert_eq!(
            published
                .seen
                .iter()
                .map(|seen| seen.identity.as_str())
                .collect::<Vec<_>>(),
            vec!["webdavis/dotfiles|workflow_run|4471|1789398987"]
        );
    }

    #[test]
    fn the_first_poll_ever_establishes_the_cursor_without_submitting_the_backlog() {
        // THE MUTANT THIS PINS: a fresh machine's first answer submitted like
        // any other tick, which pages the operator for an account's entire
        // unread backlog (up to fifty threads) in one batch through Discord,
        // the banner and the phone. `PollState::default()` IS the first-poll
        // state: no cursor has ever been published.
        let state = scratch("first-run");
        report(
            &PollState::default(),
            &state,
            GithubPolled::Listed {
                threads: vec![
                    thread("webdavis/dotfiles", "1"),
                    thread("webdavis/dotfiles", "2"),
                ],
                answer: Answer {
                    identities: Vec::new(),
                    last_modified: CURSOR.to_string(),
                    interval_secs: Some(60),
                },
            },
            NOW,
            Launch::Operator,
            &mut |event| panic!("{} was submitted on the first poll ever", event.identity),
        );
        let published = pns_adapters::read_poll_state(&state);
        assert_eq!(published.last_modified, CURSOR, "the cursor still moved");
        assert_eq!(
            published.seen.len(),
            2,
            "the backlog is in the seen-set so it is never reported later"
        );
    }

    #[test]
    fn a_state_holding_a_seen_set_and_no_cursor_is_not_the_first_poll_ever() {
        // THE MUTANT THIS PINS: "first poll" read off the cursor alone. A 200
        // that carried no `Last-Modified` publishes the batch it remembered
        // under an empty cursor, and a source that read every later tick as
        // another first poll would go silent for as long as the header did.
        let state = scratch("established-without-a-cursor");
        let stored = PollState {
            last_modified: String::new(),
            interval_secs: 60,
            seen: vec![Seen {
                identity: "webdavis/dotfiles|workflow_run|1|1789398987".to_string(),
                first_seen: NOW - 60,
            }],
        };
        let mut submitted: Vec<String> = Vec::new();
        report(
            &stored,
            &state,
            GithubPolled::Listed {
                threads: vec![thread("webdavis/dotfiles", "4471")],
                answer: Answer::default(),
            },
            NOW,
            Launch::Daemon,
            &mut |event| submitted.push(event.identity.clone()),
        );
        assert_eq!(
            submitted,
            vec!["webdavis/dotfiles|workflow_run|4471|1789398987"]
        );
    }

    #[test]
    fn a_listing_of_only_notifications_already_reported_leaves_the_seen_set_alone() {
        // THE NEGATIVE CASE FOR THE DEDUPLICATION: notifications are never
        // marked read, so the same unread thread comes back on every poll
        // forever. The seen-set is what keeps that from being a card a minute.
        let state = scratch("listed-known");
        let stored = PollState {
            last_modified: CURSOR.to_string(),
            interval_secs: 60,
            seen: vec![Seen {
                identity: "webdavis/dotfiles|workflow_run|4471|1789398987".to_string(),
                first_seen: NOW - 60,
            }],
        };
        report(
            &stored,
            &state,
            GithubPolled::Listed {
                threads: vec![thread("webdavis/dotfiles", "4471")],
                answer: Answer {
                    identities: Vec::new(),
                    last_modified: CURSOR.to_string(),
                    interval_secs: Some(60),
                },
            },
            NOW,
            Launch::Daemon,
            &mut |event| panic!("{} was submitted a second time", event.identity),
        );
        assert_eq!(
            pns_adapters::read_poll_state(&state),
            PollState::default(),
            "nothing moved, so nothing was written and the stored state stands"
        );
    }

    #[test]
    fn a_listing_submits_oldest_first_and_drops_what_it_maps_no_kind_to() {
        // THE MUTANT THIS PINS: the API's own newest-first order carried
        // through, which puts the run that finished LAST at the top of the
        // channel. The unmapped thread is here as the other half: `assign` is
        // most of a busy account's notifications and must reach nothing.
        let state = scratch("ordering");
        let mut submitted: Vec<String> = Vec::new();
        let unmapped = pns_domain::github::notifications::NotificationThread {
            reason: "assign".to_string(),
            subject_type: "Issue".to_string(),
            ..thread("webdavis/dotfiles", "9999")
        };
        report(
            &a_prior_poll(),
            &state,
            GithubPolled::Listed {
                // As the API lists them: newest first.
                threads: vec![
                    thread("webdavis/dotfiles", "3"),
                    unmapped,
                    thread("webdavis/dotfiles", "2"),
                    thread("webdavis/dotfiles", "1"),
                ],
                answer: Answer {
                    identities: Vec::new(),
                    last_modified: CURSOR.to_string(),
                    interval_secs: Some(60),
                },
            },
            NOW,
            Launch::Daemon,
            &mut |event| submitted.push(event.identity.clone()),
        );
        assert_eq!(
            submitted,
            vec![
                "webdavis/dotfiles|workflow_run|1|1789398987",
                "webdavis/dotfiles|workflow_run|2|1789398987",
                "webdavis/dotfiles|workflow_run|3|1789398987",
            ]
        );
    }
}
