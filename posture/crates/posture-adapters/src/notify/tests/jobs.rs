//! The `[jobs]` half of posture's config: the launchd label of each job it
//! installs, and when the two daily ones fire.

use super::*;
use crate::test_sandbox::Sandbox;
use posture_domain::DailyTime;

/// The labels the `[jobs]` half of one config file states.
fn labels(text: &str) -> AgentLabels {
    toml::from_str::<schema::File>(text)
        .expect("a usable config file")
        .jobs
        .into_labels()
}

#[test]
fn a_stated_job_label_is_the_one_the_tool_uses_and_the_rest_keep_their_defaults() {
    let configured = labels(
        r#"
        [notify]
        mode = "off"
        [jobs]
        alert = "com.example.osquery-results-alerter"
        digest = "com.example.roundup"
        "#,
    );
    assert_eq!(
        configured.label(Agent::Alert),
        "com.example.osquery-results-alerter"
    );
    assert_eq!(configured.label(Agent::Digest), "com.example.roundup");
    assert_eq!(configured.label(Agent::Poll), "dev.posture.poll");
}

#[test]
fn a_file_naming_no_job_leaves_every_label_at_its_shipped_default() {
    assert_eq!(
        labels("[notify]\nmode = \"off\"\n"),
        AgentLabels::default(),
        "an absent [jobs] table is the shipped posture"
    );
    assert_eq!(
        labels("[notify]\nmode = \"off\"\n[jobs]\nalert = \"\"\n"),
        AgentLabels::default(),
        "an empty label is no label, not a job with no name"
    );
}

#[test]
fn a_job_this_build_does_not_run_leaves_every_label_it_does_run_alone() {
    let text = "[notify]\nmode = \"off\"\n[jobs]\nuptime_watchdog = \"com.example.x\"\n";
    assert_eq!(labels(text), AgentLabels::default());
    assert!(
        warnings_of(text)
            .iter()
            .any(|warning| warning.contains("jobs.uptime_watchdog")),
        "{:?}",
        warnings_of(text)
    );
}

#[test]
fn the_labels_the_watchdog_searches_for_come_off_the_config_file_on_disk() {
    let home = Sandbox::new("jobs");
    let directory = home.join(".config/posture");
    std::fs::create_dir_all(&directory).expect("a sandbox home");
    std::fs::write(
        directory.join("config.toml"),
        "[notify]\nmode = \"off\"\n[jobs]\nheartbeat = \"com.example.pulse\"\n",
    )
    .expect("a config file");
    let configured = agent_labels(home.path());
    assert_eq!(configured.label(Agent::Heartbeat), "com.example.pulse");
}

/// The daily times the `[jobs.daily]` half of one config file states, with
/// whatever it says was ignored.
fn daily(text: &str) -> (DailyTimes, Vec<String>) {
    toml::from_str::<schema::File>(text)
        .expect("a usable config file")
        .jobs
        .times()
}

#[test]
fn a_stated_daily_time_is_when_that_job_fires_and_the_other_keeps_its_default() {
    let (times, warnings) =
        daily("[notify]\nmode = \"off\"\n[jobs.daily]\nheartbeat = \"07:30\"\n");
    assert_eq!(
        times,
        DailyTimes {
            heartbeat: DailyTime {
                hour: 7,
                minute: 30
            },
            ..DailyTimes::default()
        }
    );
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_daily_time_that_is_not_a_time_is_named_and_the_shipped_one_is_used() {
    let (times, warnings) = daily("[notify]\nmode = \"off\"\n[jobs.daily]\ndigest = \"6pm\"\n");
    assert_eq!(times, DailyTimes::default());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("jobs.daily.digest"), "{warnings:?}");
    assert!(warnings[0].contains("HH:MM"), "{warnings:?}");
}

#[test]
fn an_unread_daily_key_is_reported_without_voiding_the_times_beside_it() {
    let text = "[notify]\nmode = \"off\"\n[jobs.daily]\nweekly = \"03:00\"\n";
    assert_eq!(daily(text).0, DailyTimes::default());
    assert!(
        warnings_of(text)
            .iter()
            .any(|warning| warning.contains("jobs.daily.weekly")),
        "{:?}",
        warnings_of(text)
    );
}

#[test]
fn the_daily_times_and_the_labels_come_off_the_same_file_on_disk() {
    let home = Sandbox::new("job-settings");
    let directory = home.join(".config/posture");
    std::fs::create_dir_all(&directory).expect("a sandbox home");
    std::fs::write(
        directory.join("config.toml"),
        "[notify]\nmode = \"off\"\n[jobs]\ndigest = \"com.example.roundup\"\n[jobs.daily]\ndigest = \"21:05\"\n",
    )
    .expect("a config file");
    let settings = job_settings(home.path());
    assert_eq!(settings.labels.label(Agent::Digest), "com.example.roundup");
    assert_eq!(
        settings.daily.digest,
        DailyTime {
            hour: 21,
            minute: 5
        }
    );
    assert!(settings.warnings.is_empty(), "{:?}", settings.warnings);
}
