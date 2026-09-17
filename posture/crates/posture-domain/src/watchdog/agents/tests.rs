use super::*;
use crate::AgentLabels;
/// The shipped labels of two jobs, which the judgment only ever repeats back.
const HEARTBEAT: &str = "dev.posture.heartbeat";
const DIGEST: &str = "dev.posture.digest";

fn loaded(runs: Option<u64>, exit: Option<&str>) -> AgentReading<'_> {
    AgentReading::Loaded {
        runs,
        exit: AgentExit::from_field(exit),
    }
}
#[test]
fn every_shipped_label_names_posture_and_the_five_watched_jobs_exclude_the_watchdog() {
    let labels = AgentLabels::default();
    assert_eq!(
        Agent::ALL.map(|agent| labels.label(agent).to_owned()),
        [
            "dev.posture.watchdog",
            "dev.posture.alert",
            "dev.posture.poll",
            "dev.posture.funnel",
            "dev.posture.digest",
            "dev.posture.heartbeat"
        ]
    );
    assert!(!Agent::MONITORED.contains(&Agent::Watchdog));
    for agent in Agent::MONITORED {
        assert!(Agent::ALL.contains(&agent));
    }
}

#[test]
fn a_configured_label_replaces_the_default_for_that_job_alone() {
    let mut labels = AgentLabels::default();
    labels.set(Agent::Digest, "com.example.roundup".to_owned());
    assert_eq!(labels.label(Agent::Digest), "com.example.roundup");
    assert_eq!(labels.label(Agent::Alert), "dev.posture.alert");
    assert!(labels.all().any(|label| label == "com.example.roundup"));
}

#[test]
fn an_unloaded_job_is_named_by_its_configured_label_and_keeps_no_state() {
    let mut labels = AgentLabels::default();
    labels.set(Agent::Alert, "com.example.alert".to_owned());
    for agent in Agent::MONITORED {
        let label = labels.label(agent);
        let result = judge_agent(label, AgentReading::Unloaded, AgentState::default());
        assert_eq!(result.state, None);
        assert_eq!(
            result.problem,
            Some(format!("LaunchAgent not loaded: {label}"))
        );
    }
    assert_eq!(
        judge_agent(
            labels.label(Agent::Alert),
            AgentReading::Unloaded,
            AgentState::default()
        )
        .problem
        .as_deref(),
        Some("LaunchAgent not loaded: com.example.alert")
    );
}
#[test]
fn crash_streaks_require_a_new_failure_but_frozen_failures_keep_their_alarm() {
    let first = judge_agent(
        HEARTBEAT,
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
        judge_agent(HEARTBEAT, loaded(Some(1), Some("42")), prior).state,
        Some(prior)
    );
    for runs in [Some(2), Some(0), None] {
        let second = judge_agent(HEARTBEAT, loaded(runs, Some("42")), prior);
        assert_eq!(second.state, Some(AgentState { runs, streak: 2 }));
        assert_eq!(
            second.problem.as_deref(),
            Some(
                "LaunchAgent crash-looping (last exit 42, 2 failing re-runs): dev.posture.heartbeat"
            )
        );
    }
    let prior = AgentState {
        runs: Some(2),
        streak: 2,
    };
    assert!(
        judge_agent(HEARTBEAT, loaded(Some(2), Some("42")), prior)
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
        let result = judge_agent(DIGEST, loaded(Some(2), Some(code)), prior);
        assert_eq!(result.state.unwrap().streak, 0);
        assert_eq!(result.problem, None);
    }
    for code in ["prefix (never exited)", "(never exited) suffix", "secret"] {
        let result = judge_agent(DIGEST, loaded(Some(2), Some(code)), prior);
        assert_eq!(result.state.unwrap().streak, 8);
        assert_eq!(
            result.problem.as_deref(),
            Some(
                "LaunchAgent exit state is unreadable (unexpected last-exit-code value): dev.posture.digest"
            )
        );
    }
    let absent = judge_agent(DIGEST, loaded(None, None), prior);
    assert_eq!(
        absent.state,
        Some(AgentState {
            runs: None,
            streak: 8
        })
    );
    assert!(absent.problem.unwrap().contains("no last-exit-code field"));
    let numeric = judge_agent(DIGEST, loaded(Some(2), Some(" -0042 secret")), prior);
    assert_eq!(
        numeric.problem.as_deref(),
        Some("LaunchAgent crash-looping (last exit -0042, 9 failing re-runs): dev.posture.digest")
    );
}
