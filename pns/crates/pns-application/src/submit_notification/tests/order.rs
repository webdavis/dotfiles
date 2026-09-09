use super::{delivered_decision, event, missed_decision, run, submission};
use pns_domain::Overrides;

// --- the order itself ---------------------------------------------------

#[test]
fn the_records_are_written_in_the_order_the_event_path_states() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    assert_eq!(
        run(submission(&event, &decision, &overrides)),
        [
            "marker(live)",
            "news(Done)",
            "lease",
            "activity",
            "replay",
            "edge",
            "pulse(Done)",
            "clear",
            "tick",
        ]
    );
}

#[test]
fn the_tail_begins_with_the_marker_after_delivery() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    assert_eq!(steps.first().map(String::as_str), Some("marker(live)"));
}

#[test]
fn the_journal_is_written_before_the_activity_ring() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let journal = steps.iter().position(|step| step == "journal").unwrap();
    let activity = steps.iter().position(|step| step == "activity").unwrap();
    assert!(journal < activity, "{steps:?}");
}

#[test]
fn the_catch_up_runs_after_both_records_and_before_the_pulse() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let activity = steps.iter().position(|step| step == "activity").unwrap();
    let replay = steps.iter().position(|step| step == "replay").unwrap();
    let pulse = steps
        .iter()
        .position(|step| step.starts_with("pulse"))
        .unwrap();
    assert!(activity < replay && replay < pulse, "{steps:?}");
}

#[test]
fn the_pulse_goes_after_every_record_the_operator_might_be_waiting_on() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let pulse = steps
        .iter()
        .position(|step| step.starts_with("pulse"))
        .unwrap();
    for earlier in ["activity", "replay"] {
        let at = steps.iter().position(|step| step == earlier).unwrap();
        assert!(at < pulse, "{earlier} ran after the pulse: {steps:?}");
    }
}

#[test]
fn the_lights_tick_is_registered_last() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    assert_eq!(steps.last().map(String::as_str), Some("tick"));
}

#[test]
fn a_missed_event_is_journaled_before_its_marker_and_keeps_held_lamps() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    assert_eq!(
        run(submission(&event, &decision, &overrides)),
        [
            "journal",
            "marker(live)",
            "news(Done)",
            "lease",
            "activity",
            "tick"
        ]
    );
}

#[test]
fn outcome_contract_the_tail_does_not_append_a_second_decision() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    assert!(
        !steps.iter().any(|step| step.starts_with("decision")),
        "{steps:?}"
    );
}
