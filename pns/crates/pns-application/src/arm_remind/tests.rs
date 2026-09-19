use super::ArmRemind;
use crate::clear_remind;
mod fixture;
use fixture::{Recorder, event};

#[test]
fn arming_clears_the_previous_answer_before_publication_and_schedules_no_private_text() {
    let records = Recorder::default();
    ArmRemind {
        records: &records,
        jobs: &records,
    }
    .run(
        "session",
        &event(),
        60,
        || Some(100),
        |_| panic!("unexpected warning"),
    );
    assert_eq!(*records.steps.borrow(), ["clear", "publish", "schedule"]);
    let jobs = records.jobs.borrow();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "remind:session");
    assert_eq!(
        (jobs[0].due, jobs[0].until, jobs[0].every),
        (160, 220, None)
    );
    assert_eq!(jobs[0].unless_marker.as_deref(), Some("remind-session"));
    assert_eq!(jobs[0].args, ["remind"]);
    assert_eq!(
        records.records.borrow()[0].detail,
        "private approval question"
    );
    assert_eq!(records.records.borrow()[0].armed, 100);
}

#[test]
fn a_failed_marker_clear_warns_but_preserves_the_existing_publication_attempt() {
    let records = Recorder {
        fail: "clear",
        ..Default::default()
    };
    let mut warnings = Vec::new();
    ArmRemind {
        records: &records,
        jobs: &records,
    }
    .run(
        "session",
        &event(),
        60,
        || Some(100),
        |line| warnings.push(line.to_string()),
    );
    assert_eq!(*records.steps.borrow(), ["clear", "publish", "schedule"]);
    assert_eq!(
        warnings,
        [
            "pns: a previous approval's answered marker could not be cleared (clear); this approval will not be nudged"
        ]
    );
}

#[test]
fn a_failed_remind_publication_never_registers_a_job() {
    let records = Recorder {
        fail: "publish",
        ..Default::default()
    };
    let mut warnings = Vec::new();
    ArmRemind {
        records: &records,
        jobs: &records,
    }
    .run(
        "session",
        &event(),
        60,
        || Some(100),
        |line| warnings.push(line.to_string()),
    );
    assert_eq!(*records.steps.borrow(), ["clear", "publish"]);
    assert_eq!(
        warnings,
        [
            "pns: the reminder record could not be written (publish); this approval will not be nudged"
        ]
    );
}

#[test]
fn a_refused_schedule_rolls_back_the_record_and_reports_both_rollback_outcomes() {
    for (fail, ending) in [
        ("schedule", "its record is dropped"),
        (
            "schedule,drop",
            "and its record could not be dropped either",
        ),
    ] {
        let records = Recorder {
            fail,
            ..Default::default()
        };
        let mut warnings = Vec::new();
        ArmRemind {
            records: &records,
            jobs: &records,
        }
        .run(
            "session",
            &event(),
            60,
            || Some(100),
            |line| warnings.push(line.to_string()),
        );
        assert_eq!(
            *records.steps.borrow(),
            ["clear", "publish", "schedule", "drop"]
        );
        assert_eq!(
            warnings,
            [format!(
                "pns: the reminder could not be scheduled (schedule); this approval will not be nudged, {ending}"
            )]
        );
    }
}

#[test]
fn resolving_a_batch_marks_first_and_drops_even_when_marking_failed() {
    for fail in ["", "mark"] {
        let records = Recorder {
            fail,
            ..Default::default()
        };
        let mut warnings = Vec::new();
        clear_remind(&records, "session", |line| warnings.push(line.to_string()));
        assert_eq!(*records.steps.borrow(), ["mark", "drop"]);
        assert_eq!(
            warnings,
            if fail.is_empty() {
                vec![]
            } else {
                vec!["pns: an answered marker could not be written (mark)".to_string()]
            }
        );
    }
}

#[test]
fn a_disabled_remind_and_an_invalid_session_never_read_the_clock() {
    // THE PRODUCER'S NAME DECIDES NOTHING HERE. Whether this call arms is
    // resolved before the run, and a delay of zero is the whole of what this
    // layer reads as off.
    let records = Recorder::default();
    for agent in ["codex", "", "other"] {
        let mut event = event();
        event.agent = agent.into();
        ArmRemind {
            records: &records,
            jobs: &records,
        }
        .run(
            "session",
            &event,
            0,
            || panic!("clock"),
            |_| panic!("warning"),
        );
    }
    for (session, after) in [("session", 0), ("../session", 60), ("", 60)] {
        ArmRemind {
            records: &records,
            jobs: &records,
        }
        .run(
            session,
            &event(),
            after,
            || panic!("clock"),
            |_| panic!("warning"),
        );
    }
    ArmRemind {
        records: &records,
        jobs: &records,
    }
    .run("session", &event(), 60, || None, |_| panic!("warning"));
    clear_remind(&records, "../session", |_| panic!("warning"));
    assert!(records.steps.borrow().is_empty());
}
