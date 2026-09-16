mod tests {
    use super::super::*;
    use pns_adapters::GithubPolled;
    use pns_domain::channel_map::{ChannelMap, DEFAULT_KEY, channel_for};
    use pns_domain::github::poll::Seen;
    use pns_domain::github::{GithubKind, GithubOutcome};

    const NOW: u64 = 1_700_000_000;
    const CURSOR: &str = "Thu, 25 Oct 2026 15:16:27 GMT";

    fn event(repo: &str, identity: &str) -> GithubEvent {
        GithubEvent {
            repo: repo.to_string(),
            kind: GithubKind::WorkflowRun,
            outcome: GithubOutcome::Neutral,
            title: "lint".to_string(),
            url: format!("https://github.com/{repo}"),
            identity: identity.to_string(),
            occurred_at: 1_789_398_987,
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pns-github-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&path).expect("the scratch directory");
        path
    }

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
            &PollState::default(),
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
        assert_eq!(submitted, vec!["webdavis/dotfiles|workflow_run|4471"]);
        let published = pns_adapters::read_poll_state(&state);
        assert_eq!(published.last_modified, CURSOR);
        assert_eq!(published.interval_secs, 60);
        assert_eq!(
            published
                .seen
                .iter()
                .map(|seen| seen.identity.as_str())
                .collect::<Vec<_>>(),
            vec!["webdavis/dotfiles|workflow_run|4471"]
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
                identity: "webdavis/dotfiles|workflow_run|4471".to_string(),
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
            &PollState::default(),
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
                "webdavis/dotfiles|workflow_run|1",
                "webdavis/dotfiles|workflow_run|2",
                "webdavis/dotfiles|workflow_run|3",
            ]
        );
    }

    /// One thread as the wire states it, for the two tests that need one.
    fn thread(
        repo: &str,
        subject_id: &str,
    ) -> pns_domain::github::notifications::NotificationThread {
        pns_domain::github::notifications::NotificationThread {
            id: "20111".to_string(),
            reason: "ci_activity".to_string(),
            subject_type: "CheckSuite".to_string(),
            subject_title: "lint".to_string(),
            subject_url: format!("https://api.github.com/repos/{repo}/check-suites/{subject_id}"),
            repo_full_name: repo.to_string(),
            repo_html_url: format!("https://github.com/{repo}"),
            updated_at: 1_789_398_987,
        }
    }

    // --- the envelope, and where it lands --------------------------------

    #[test]
    fn the_envelope_names_the_repository_as_the_project_and_carries_the_extension() {
        let request =
            request_for(&event("webdavis/dotfiles", "id-1"), NOW).expect("it is spellable");
        assert_eq!(
            request.context.project.as_deref(),
            Some("webdavis/dotfiles"),
            "the FULL name, owner included, is what the channel map keys on"
        );
        assert_eq!(request.producer.as_str(), "github");
        assert_eq!(request.detail, "lint");
        assert_eq!(request.occurred_at, Some(1_789_398_987));
        // NO CLASS, because GitHub is work rather than machine health: a lint
        // failure is not a posture page and must not bypass a Focus.
        assert_eq!(request.class, None);
        assert_eq!(
            request.route, None,
            "the ordinary route, not the urgent one"
        );
        assert_eq!(
            request.signal,
            pns_protocol::Signal::Observation,
            "a polled notification is something that happened, not a turn waiting"
        );
        assert_eq!(
            pns_adapters::github_event(&request.extensions),
            Ok(Some(event("webdavis/dotfiles", "id-1")))
        );
    }

    #[test]
    fn the_request_id_is_the_events_identity_so_the_ledger_is_a_second_guard() {
        // THE MUTANT THIS PINS: a fresh id per submission, which would leave
        // a repeat (the seen-set expiring, or a crash between the batch and
        // the state write) delivered twice instead of answered as existing.
        let one = event("webdavis/dotfiles", "webdavis/dotfiles|workflow_run|4471");
        assert_eq!(
            request_for(&one, NOW).map(|request| request.request_id.as_str().to_string()),
            Some("webdavis/dotfiles|workflow_run|4471".to_string())
        );
    }

    #[test]
    fn an_instant_the_parse_could_not_read_falls_back_to_now() {
        // Zero would read downstream as work that ran since 1970.
        let unreadable = GithubEvent {
            occurred_at: 0,
            ..event("webdavis/dotfiles", "id-1")
        };
        assert_eq!(
            request_for(&unreadable, NOW).unwrap().occurred_at,
            Some(NOW)
        );
    }

    #[test]
    fn an_identity_no_request_id_can_carry_submits_nothing_rather_than_panicking() {
        // A repository name is the operator's and a title is GitHub's, so the
        // identity is remote text: past the id's 128 characters, or carrying
        // something outside visible ASCII, it is dropped.
        for identity in [String::new(), "x".repeat(200), "a b".to_string()] {
            assert!(
                request_for(
                    &GithubEvent {
                        identity: identity.clone(),
                        ..event("webdavis/dotfiles", "unused")
                    },
                    NOW
                )
                .is_none(),
                "case {identity:?}"
            );
        }
    }

    #[test]
    fn a_mapped_repository_reaches_its_own_channel_and_an_unmapped_one_the_catch_all() {
        // THE TWO HALVES COMPOSED: the envelope names the repository as the
        // project, and the shared map is what turns a project into a channel.
        // This is the behaviour sentence the source was asked for, and it
        // runs over the SAME lookup the Discord destination uses rather than
        // a second mapping of this source's own.
        let channels: ChannelMap = [
            (DEFAULT_KEY, "github-notifications"),
            ("dotfiles", "dotfiles-dev"),
            ("webdavis/pns", "pns-dev"),
        ]
        .iter()
        .map(|(key, channel)| ((*key).to_string(), (*channel).to_string()))
        .collect();
        let landed = |repo: &str| {
            let request = request_for(&event(repo, "id-1"), NOW).expect("it is spellable");
            let project = request.context.project.clone().unwrap_or_default();
            channel_for(&channels, "", &project, "pns-events").map(str::to_string)
        };
        assert_eq!(
            landed("webdavis/dotfiles"),
            Some("dotfiles-dev".to_string()),
            "the bare entry answers a full name nobody keyed"
        );
        assert_eq!(
            landed("webdavis/pns"),
            Some("pns-dev".to_string()),
            "and a full-name entry beats it"
        );
        assert_eq!(
            landed("someone/unmapped"),
            Some("github-notifications".to_string()),
            "an unmapped repository is a delivered event, never a dropped one"
        );
    }
}
