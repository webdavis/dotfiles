use super::*;
use crate::support::STUB_CHANNELS;

/// The stub channels with a delay set, and the producer entry only when asked
/// for. THE DEFAULT IS OFF: a delay alone arms nothing on either path.
fn remind_config(delay_secs: u64, producer_entry: bool) -> String {
    let entry = if producer_entry {
        "[producer.posture]\nremind = true\n"
    } else {
        ""
    };
    format!(
        "{}{entry}[remind]\ndelay = \"{delay_secs}s\"\n",
        STUB_CHANNELS
    )
}

/// One blocked approval a producer submitted as JSON, stating `remind` the way
/// `value` spells it (`None` for a request that says nothing).
fn blocked(sandbox: &Sandbox, value: Option<serde_json::Value>) -> Output {
    let mut request = request();
    request.state = State::Blocked;
    request.session = Some(Name::new("s1").unwrap());
    let mut encoded: serde_json::Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
    if let Some(value) = value {
        encoded["remind"] = value;
    }
    invoke(sandbox, &serde_json::to_string(&encoded).unwrap())
}

fn remind_record(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.path("state/remind/s1.pending")
}

/// How long the nudge this run registered waits, off the spool entry's own due
/// and the record's armed second, which is `remind_switch.rs`'s reading of the
/// same two files for the flag path.
fn armed_delay(sandbox: &Sandbox) -> u64 {
    let entry = std::fs::read_to_string(sandbox.path("state/daemon/remind:s1")).expect("a job");
    let due: u64 = entry
        .split('\t')
        .find_map(|part| part.strip_prefix("due="))
        .unwrap_or_else(|| panic!("no `due=` in {entry}"))
        .parse()
        .expect("a due second");
    let record = std::fs::read_to_string(remind_record(sandbox)).expect("a record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("the record is JSON");
    due - parsed["armed"].as_u64().expect("an armed second")
}

#[test]
fn a_request_asking_for_the_configured_delay_arms_at_it() {
    let sandbox = Sandbox::new("json-remind-true");
    sandbox.write_config(&remind_config(300, false));

    let output = blocked(&sandbox, Some(serde_json::json!(true)));
    assert_eq!(result(&output).status, Status::Accepted);
    assert!(
        remind_record(&sandbox).exists(),
        "`\"remind\": true` arms what the producer table never asked for"
    );
    assert_eq!(armed_delay(&sandbox), 300, "at the configured delay");
}

#[test]
fn a_requests_own_duration_beats_the_configured_delay() {
    let sandbox = Sandbox::new("json-remind-duration");
    sandbox.write_config(&remind_config(60, false));

    let output = blocked(&sandbox, Some(serde_json::json!("5m")));
    assert_eq!(result(&output).status, Status::Accepted);
    assert_eq!(armed_delay(&sandbox), 300);
}

#[test]
fn a_request_can_disarm_a_reminder_the_producer_table_asked_for() {
    let sandbox = Sandbox::new("json-remind-false");
    sandbox.write_config(&remind_config(300, true));

    let output = blocked(&sandbox, Some(serde_json::json!(false)));
    assert_eq!(result(&output).status, Status::Accepted);
    assert!(
        !remind_record(&sandbox).exists(),
        "`\"remind\": false` beats the producer's own entry"
    );
    assert!(
        !sandbox.path("state/daemon/remind:s1").exists(),
        "and registers no job either"
    );
}

#[test]
fn a_request_that_says_nothing_falls_through_to_the_producer_table_and_then_off() {
    let sandbox = Sandbox::new("json-remind-absent");
    sandbox.write_config(&remind_config(300, false));

    assert_eq!(result(&blocked(&sandbox, None)).status, Status::Accepted);
    assert!(
        !remind_record(&sandbox).exists(),
        "a delay alone arms nothing"
    );

    sandbox.write_config(&remind_config(300, true));
    assert_eq!(result(&blocked(&sandbox, None)).status, Status::Accepted);
    assert_eq!(
        armed_delay(&sandbox),
        300,
        "and the producer's own entry is what arms it"
    );
}

#[test]
fn a_state_no_approval_waits_on_arms_nothing_however_the_request_asks() {
    // The producer table states that producer's APPROVALS, so an observation
    // it sends is not a wait to nudge about however the field is spelled.
    let sandbox = Sandbox::new("json-remind-not-blocked");
    sandbox.write_config(&remind_config(300, true));

    let mut request = request();
    request.session = Some(Name::new("s1").unwrap());
    let mut encoded: serde_json::Value = serde_json::from_str(&request.encode().unwrap()).unwrap();
    encoded["remind"] = serde_json::json!(true);
    let output = invoke(&sandbox, &serde_json::to_string(&encoded).unwrap());

    assert_eq!(result(&output).status, Status::Accepted);
    assert!(!remind_record(&sandbox).exists());
}
