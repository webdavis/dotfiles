use super::*;
fn loaded(runs: Option<u64>, exit: Option<&str>) -> AgentReading<'_> {
    AgentReading::Loaded {
        runs,
        exit: AgentExit::from_field(exit),
    }
}
#[test]
fn the_six_watched_agents_keep_their_labels_and_unloaded_state_is_not_retained() {
    assert_eq!(
        Agent::ALL.map(Agent::label),
        [
            "com.webdavis.osquery-results-alerter",
            "com.webdavis.osquery-firewall-gatekeeper-monitor",
            "com.webdavis.osquery-alert-drainer",
            "com.webdavis.osquery-digest",
            "com.webdavis.osquery-heartbeat",
            "com.webdavis.osquery-tailscale-monitor"
        ]
    );
    for agent in Agent::ALL {
        let result = judge_agent(agent, AgentReading::Unloaded, AgentState::default());
        assert_eq!(result.state, None);
        assert_eq!(
            result.problem,
            Some(format!("LaunchAgent not loaded: {}", agent.label()))
        );
    }
}
#[test]
fn crash_streaks_require_a_new_failure_but_frozen_failures_keep_their_alarm() {
    let first = judge_agent(
        Agent::Heartbeat,
        loaded(Some(1), Some("42")),
        AgentState::default(),
    );
    assert_eq!(
        first.state,
        Some(AgentState {
            runs: Some(1),
            streak: 1
        })
    );
    assert_eq!(first.problem, None);
    let prior = first.state.unwrap();
    assert_eq!(
        judge_agent(Agent::Heartbeat, loaded(Some(1), Some("42")), prior).state,
        Some(prior)
    );
    for runs in [Some(2), Some(0), None] {
        let second = judge_agent(Agent::Heartbeat, loaded(runs, Some("42")), prior);
        assert_eq!(second.state, Some(AgentState { runs, streak: 2 }));
        assert_eq!(
            second.problem.as_deref(),
            Some(
                "LaunchAgent crash-looping (last exit 42, 2 failing re-runs): com.webdavis.osquery-heartbeat"
            )
        );
    }
    let prior = AgentState {
        runs: Some(2),
        streak: 2,
    };
    assert!(
        judge_agent(Agent::Heartbeat, loaded(Some(2), Some("42")), prior)
            .problem
            .is_some()
    );
}
#[test]
fn exit_classification_preserves_numeric_prefixes_and_refuses_unanchored_sentinels() {
    let prior = AgentState {
        runs: Some(1),
        streak: 8,
    };
    for code in ["0", "-0", " 00 rest", " (never exited)\t"] {
        let result = judge_agent(Agent::Digest, loaded(Some(2), Some(code)), prior);
        assert_eq!(result.state.unwrap().streak, 0);
        assert_eq!(result.problem, None);
    }
    for code in ["prefix (never exited)", "(never exited) suffix", "secret"] {
        let result = judge_agent(Agent::Digest, loaded(Some(2), Some(code)), prior);
        assert_eq!(result.state.unwrap().streak, 8);
        assert_eq!(
            result.problem.as_deref(),
            Some(
                "LaunchAgent exit state is unreadable (unexpected last-exit-code value): com.webdavis.osquery-digest"
            )
        );
    }
    let absent = judge_agent(Agent::Digest, loaded(None, None), prior);
    assert_eq!(
        absent.state,
        Some(AgentState {
            runs: None,
            streak: 8
        })
    );
    assert!(absent.problem.unwrap().contains("no last-exit-code field"));
    let numeric = judge_agent(Agent::Digest, loaded(Some(2), Some(" -0042 secret")), prior);
    assert_eq!(
        numeric.problem.as_deref(),
        Some(
            "LaunchAgent crash-looping (last exit -0042, 9 failing re-runs): com.webdavis.osquery-digest"
        )
    );
}
