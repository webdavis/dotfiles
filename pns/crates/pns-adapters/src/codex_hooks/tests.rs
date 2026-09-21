use super::*;

const HOME: &str = "/home/tester";
const BINARY: &str = "/home/tester/.cargo/bin/pns";

fn merged(base: Value) -> Value {
    merge_codex_hooks(&base, HOME, BINARY).expect("the merge must succeed")
}

fn commands(document: &Value, event: &str) -> Vec<String> {
    document["hooks"][event]
        .as_array()
        .expect("the event must carry a list of entries")
        .iter()
        .flat_map(|entry| entry["hooks"].as_array().expect("entry handlers").iter())
        .map(|handler| handler["command"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn an_empty_document_gains_one_handler_on_each_of_the_four_events() {
    let document = merged(json!({"hooks": {}}));
    assert_eq!(
        commands(&document, "Stop"),
        vec![format!("PNS_PRODUCER=codex {BINARY} hook stop")]
    );
    assert_eq!(
        commands(&document, "PermissionRequest"),
        vec![format!("PNS_PRODUCER=codex {BINARY} hook blocked --remind")]
    );
    for event in ["PostToolUse", "Interrupt"] {
        assert_eq!(
            commands(&document, event),
            vec![format!("PNS_PRODUCER=codex {BINARY} hook resolved")],
            "{event}"
        );
    }
}

#[test]
fn a_second_merge_over_the_first_changes_nothing() {
    let once = merged(json!({"hooks": {}}));
    assert_eq!(merged(once.clone()), once);
}

#[test]
fn every_legacy_spelling_is_rewritten_in_place() {
    for legacy in [
        format!("PNS_PRODUCER=codex {HOME}/.cargo/bin/pns hook stop"),
        format!("PNS_AGENT=codex {HOME}/.cargo/bin/pns hook stop"),
        format!("RELAY_AGENT=codex {HOME}/.cargo/bin/pns hook stop"),
        format!("PNS_PRODUCER=codex {HOME}/.local/libexec/pns/pns hook stop"),
        format!("PNS_AGENT=codex {HOME}/.local/libexec/pns/pns hook stop"),
        format!("RELAY_AGENT=codex {HOME}/.local/libexec/pns/pns hook stop"),
        format!("RELAY_AGENT=codex {HOME}/.local/bin/relay-agent.sh done"),
        format!("RELAY_AGENT=codex {HOME}/.local/libexec/pns/codex-hooks/relay-agent.sh done"),
        format!("RELAY_AGENT=codex {HOME}/.local/libexec/pns/hooks/relay-agent.sh done"),
        format!("PNS_AGENT=codex {HOME}/.local/libexec/pns/hooks/relay-agent.sh done"),
    ] {
        let document = merged(json!({"hooks": {
            "Stop": [{"hooks": [{"type": "command", "command": legacy}]}]
        }}));
        assert_eq!(
            commands(&document, "Stop"),
            vec![format!("PNS_PRODUCER=codex {BINARY} hook stop")],
            "{legacy}"
        );
    }
}

#[test]
fn a_handler_belonging_to_another_home_is_foreign_and_survives() {
    let foreign = "PNS_AGENT=codex /home/other/.cargo/bin/pns hook stop";
    let document = merged(json!({"hooks": {
        "Stop": [{"hooks": [{"type": "command", "command": foreign}]}]
    }}));
    assert_eq!(
        commands(&document, "Stop"),
        vec![
            foreign.to_string(),
            format!("PNS_PRODUCER=codex {BINARY} hook stop")
        ]
    );
}

#[test]
fn a_foreign_handler_beside_a_pns_one_keeps_its_place_in_the_entry() {
    let herdr = "bash herdr-agent-state.sh session";
    let document = merged(json!({"hooks": {
        "Stop": [{"hooks": [
            {"type": "command", "command": herdr},
            {"type": "command", "command": format!("PNS_AGENT=codex {HOME}/.local/libexec/pns/pns hook stop")}
        ]}]
    }}));
    assert_eq!(
        commands(&document, "Stop"),
        vec![
            herdr.to_string(),
            format!("PNS_PRODUCER=codex {BINARY} hook stop")
        ]
    );
    assert_eq!(
        document["hooks"]["Stop"].as_array().expect("entries").len(),
        1
    );
}

#[test]
fn a_duplicate_owner_collapses_and_the_entry_it_emptied_is_dropped() {
    let legacy = format!("PNS_AGENT=codex {HOME}/.local/libexec/pns/pns hook stop");
    let document = merged(json!({"hooks": {
        "Stop": [
            {"hooks": [{"type": "command", "command": legacy}]},
            {"hooks": [{"type": "command", "command": format!("PNS_PRODUCER=codex {BINARY} hook stop")}]}
        ]
    }}));
    assert_eq!(
        document["hooks"]["Stop"],
        json!([{"hooks": [{"type": "command", "command": format!("PNS_PRODUCER=codex {BINARY} hook stop")}]}])
    );
}

#[test]
fn an_entry_that_arrived_empty_is_somebody_elses_and_stays() {
    let document = merged(json!({"hooks": {"Stop": [{"matcher": "x", "hooks": []}]}}));
    assert_eq!(
        document["hooks"]["Stop"],
        json!([
            {"matcher": "x", "hooks": []},
            {"hooks": [{"type": "command", "command": format!("PNS_PRODUCER=codex {BINARY} hook stop")}]}
        ])
    );
}

#[test]
fn two_owners_whose_metadata_differs_are_refused() {
    let legacy = format!("PNS_AGENT=codex {HOME}/.local/libexec/pns/pns hook stop");
    let current = format!("PNS_PRODUCER=codex {BINARY} hook stop");
    for base in [
        // The entries differ.
        json!({"hooks": {"Stop": [
            {"hooks": [{"type": "command", "command": legacy}]},
            {"matcher": "x", "hooks": [{"type": "command", "command": current}]}
        ]}}),
        // The handlers differ.
        json!({"hooks": {"Stop": [
            {"hooks": [
                {"type": "command", "timeout": 12, "command": legacy},
                {"type": "command", "command": current}
            ]}
        ]}}),
    ] {
        let refusal =
            merge_codex_hooks(&base, HOME, BINARY).expect_err("differing metadata must be refused");
        assert!(refusal.contains("Stop"), "{refusal}");
    }
}

#[test]
fn a_document_without_an_object_hooks_field_is_refused() {
    for base in [json!([]), json!({"hooks": []}), json!("hooks")] {
        assert!(
            merge_codex_hooks(&base, HOME, BINARY).is_err(),
            "{base:?} must be refused"
        );
    }
}

#[test]
fn an_event_that_is_not_a_list_of_entries_is_refused() {
    for base in [
        json!({"hooks": {"Stop": {}}}),
        json!({"hooks": {"Stop": ["x"]}}),
        json!({"hooks": {"Stop": [{"matcher": "x"}]}}),
    ] {
        assert!(
            merge_codex_hooks(&base, HOME, BINARY).is_err(),
            "{base:?} must be refused"
        );
    }
}
