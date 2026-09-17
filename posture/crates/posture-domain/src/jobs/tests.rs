use super::*;

fn labels() -> AgentLabels {
    AgentLabels::default()
}

fn plan(agent: Agent) -> JobPlan {
    JobPlan::new(
        agent,
        &labels(),
        DailyTimes::default(),
        Path::new("/private/bin/posture"),
        Path::new("/private/home"),
    )
}

#[test]
fn every_job_runs_its_own_subcommand_under_its_own_label_and_log() {
    for agent in Agent::ALL {
        let plan = plan(agent);
        assert_eq!(plan.subcommand, agent.key());
        assert_eq!(plan.label, format!("dev.posture.{}", agent.key()));
        assert_eq!(
            plan.log,
            PathBuf::from(format!(
                "/private/home/.local/log/posture/{}.log",
                agent.key()
            ))
        );
        assert_eq!(
            plan.unit_path(Path::new("/private/home")),
            PathBuf::from(format!(
                "/private/home/Library/LaunchAgents/dev.posture.{}.plist",
                agent.key()
            ))
        );
    }
    assert_eq!(
        JobPlan::all(
            &labels(),
            DailyTimes::default(),
            Path::new("/private/bin/posture"),
            Path::new("/private/home"),
        )
        .len(),
        Agent::ALL.len(),
        "every installed job is planned"
    );
}

#[test]
fn the_scheduled_jobs_carry_the_intervals_and_daily_times_of_the_pipeline() {
    let daily = DailyTimes {
        digest: DailyTime {
            hour: 21,
            minute: 5,
        },
        heartbeat: DailyTime {
            hour: 7,
            minute: 30,
        },
    };
    let trigger = |agent| {
        JobPlan::new(
            agent,
            &labels(),
            daily,
            Path::new("/private/bin/posture"),
            Path::new("/private/home"),
        )
        .trigger
    };
    assert_eq!(trigger(Agent::Poll), Trigger::Every(60));
    assert_eq!(trigger(Agent::Funnel), Trigger::Every(60));
    assert_eq!(trigger(Agent::Alert), Trigger::Every(300));
    assert_eq!(trigger(Agent::Watchdog), Trigger::Every(900));
    assert_eq!(trigger(Agent::Digest), Trigger::Daily(daily.digest));
    assert_eq!(trigger(Agent::Heartbeat), Trigger::Daily(daily.heartbeat));
}

#[test]
fn only_the_alert_job_watches_the_result_log_and_only_the_interval_jobs_run_at_load() {
    for agent in Agent::ALL {
        let plan = plan(agent);
        assert_eq!(
            plan.watch,
            matches!(agent, Agent::Alert)
                .then(|| PathBuf::from("/private/home/.local/log/osquery/osqueryd.results.log")),
            "watch path for {}",
            agent.key()
        );
        assert_eq!(
            plan.run_at_load,
            matches!(agent, Agent::Poll | Agent::Funnel | Agent::Watchdog),
            "run at load for {}",
            agent.key()
        );
    }
}

#[test]
fn an_interval_unit_states_its_program_its_interval_and_a_usable_search_path() {
    let unit = plan(Agent::Poll).unit();
    for line in [
        "<key>Label</key>",
        "<string>dev.posture.poll</string>",
        "<string>/private/bin/posture</string>",
        "<string>poll</string>",
        "<key>StartInterval</key>",
        "<integer>60</integer>",
        "<key>RunAtLoad</key>",
        "<true/>",
        "<string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>",
        "<string>/private/home/.local/log/posture/poll.log</string>",
    ] {
        assert!(
            unit.contains(line),
            "the poll unit must carry `{line}`: {unit}"
        );
    }
    assert!(unit.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist"));
    assert!(unit.ends_with("</dict>\n</plist>\n"));
    assert!(!unit.contains("StartCalendarInterval"));
    assert!(!unit.contains("WatchPaths"));
}

#[test]
fn a_daily_unit_states_a_calendar_interval_and_no_interval_at_all() {
    let unit = plan(Agent::Digest).unit();
    assert!(unit.contains("<key>StartCalendarInterval</key>"));
    assert!(unit.contains("<key>Hour</key>\n    <integer>18</integer>"));
    assert!(unit.contains("<key>Minute</key>\n    <integer>0</integer>"));
    assert!(!unit.contains("<key>StartInterval</key>"));
    assert!(unit.contains("<false/>"));
}

#[test]
fn the_alert_unit_carries_both_its_interval_and_the_result_log_watch() {
    let unit = plan(Agent::Alert).unit();
    assert!(unit.contains("<key>WatchPaths</key>"));
    assert!(
        unit.contains("<string>/private/home/.local/log/osquery/osqueryd.results.log</string>")
    );
    assert!(unit.contains("<integer>300</integer>"));
}

#[test]
fn a_label_holding_xml_syntax_cannot_break_the_unit_it_names() {
    let mut labels = AgentLabels::default();
    labels.set(Agent::Poll, "dev.<posture>&\"one\"".to_string());
    let unit = JobPlan::new(
        Agent::Poll,
        &labels,
        DailyTimes::default(),
        Path::new("/private/bin/posture"),
        Path::new("/private/home"),
    )
    .unit();
    assert!(unit.contains("<string>dev.&lt;posture&gt;&amp;&quot;one&quot;</string>"));
    assert!(!unit.contains("<posture>"));
}

#[test]
fn a_time_of_day_is_two_digit_hours_and_minutes_or_a_named_refusal() {
    assert_eq!(
        DailyTime::parse("18:00"),
        Ok(DailyTime {
            hour: 18,
            minute: 0
        })
    );
    assert_eq!(
        DailyTime::parse("07:05"),
        Ok(DailyTime { hour: 7, minute: 5 })
    );
    for text in [
        "", "18", "18:0", "8:00", "24:00", "18:60", "-1:00", "aa:bb", "18:00:00",
    ] {
        assert_eq!(
            DailyTime::parse(text),
            Err(format!("`{text}` is not a time of day written as HH:MM")),
            "`{text}` must be refused"
        );
    }
}

#[test]
fn a_unit_that_matches_has_no_drift_whatever_its_order_and_indentation() {
    let unit = plan(Agent::Poll).unit();
    assert!(unit_drift(&unit, &unit).is_empty());
    let reordered: String = {
        let mut lines: Vec<&str> = unit.lines().collect();
        lines.reverse();
        lines.join("\n    ") + "\n\n"
    };
    assert!(
        unit_drift(&unit, &reordered).is_empty(),
        "only content differs, not order or whitespace"
    );
}

#[test]
fn drift_names_the_line_each_side_holds_alone_including_a_repeat() {
    let drift = unit_drift(
        "<key>Label</key>\n<string>dev.posture.poll</string>\n<string>same</string>\n<string>same</string>\n",
        "<key>Label</key>\n<string>com.example.poll</string>\n<string>same</string>\n",
    );
    assert_eq!(
        drift.expected,
        vec![
            "<string>dev.posture.poll</string>".to_string(),
            "<string>same</string>".to_string()
        ]
    );
    assert_eq!(
        drift.live,
        vec!["<string>com.example.poll</string>".to_string()]
    );
    assert!(!drift.is_empty());
}

#[test]
fn the_schedule_reads_as_one_line_naming_the_watch_when_there_is_one() {
    assert_eq!(plan(Agent::Poll).schedule(), "every 60s");
    assert_eq!(plan(Agent::Digest).schedule(), "daily at 18:00");
    assert_eq!(
        plan(Agent::Alert).schedule(),
        "every 300s, and on a write to /private/home/.local/log/osquery/osqueryd.results.log"
    );
}
