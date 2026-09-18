mod tests {
    use super::super::github_test_fixture::{NOW, event};
    use super::super::*;
    use pns_domain::channel_map::{ChannelMap, DEFAULT_KEY, channel_for};

    #[test]
    fn the_envelope_names_the_repository_as_the_project_and_carries_the_extension() {
        let request =
            request_for(&event("webdavis/dotfiles", "id-1"), NOW).expect("it is spellable");
        assert_eq!(
            request.project.as_deref(),
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
            request.state,
            pns_protocol::State::Observation,
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
            let project = request.project.clone().unwrap_or_default();
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
