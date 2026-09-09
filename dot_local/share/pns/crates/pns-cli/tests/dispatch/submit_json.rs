use super::*;
use pns_protocol::{Name, Request, RequestId, Signal, Status};
use std::io::Write;
use std::process::{Output, Stdio};

fn request() -> Request {
    let mut request = Request::new(
        RequestId::new("source-123").unwrap(),
        Name::new("posture").unwrap(),
        Name::new("failed").unwrap(),
        Signal::Observation,
    );
    request.detail = "original detail".into();
    request.context.project = Some("original project".into());
    request.context.branch = Some("original branch".into());
    request.route = Some(Name::new("priority").unwrap());
    request.session = Some(pns_protocol::Session {
        id: Name::new("original-session").unwrap(),
        turn: Some(7),
    });
    request.occurred_at = Some(123);
    request.elapsed_secs = Some(3);
    request.interaction = pns_protocol::Interaction::AwaitDecision;
    request
        .extensions
        .insert("source_data".into(), serde_json::json!({"unchanged": true}));
    request
}

fn invoke(sandbox: &Sandbox, input: &str) -> Output {
    invoke_command(sandbox, sandbox.pns_stateful(), input)
}

fn invoke_command(sandbox: &Sandbox, mut command: std::process::Command, input: &str) -> Output {
    for (key, leaf) in [
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_DATA_HOME", "d"),
        ("XDG_STATE_HOME", "s"),
        ("XDG_CACHE_HOME", "k"),
        ("XDG_RUNTIME_DIR", "r"),
        ("XDG_CONFIG_DIRS", "c"),
        ("XDG_DATA_DIRS", "e"),
        ("CLAUDE_CONFIG_DIR", "a"),
        ("TMPDIR", "t"),
        ("TMP", "t"),
        ("TEMP", "t"),
    ] {
        std::fs::create_dir_all(sandbox.path(leaf)).unwrap();
        command.env(key, sandbox.path(leaf));
    }
    let mut child = command
        .args(["submit", "--json"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn result(output: &Output) -> pns_protocol::ResultEnvelope {
    assert_eq!(
        stdout(output).lines().count(),
        1,
        "sole result line: {output:?}"
    );
    pns_protocol::decode_result(&output.stdout).expect("a valid result envelope")
}

#[test]
fn json_submission_commits_original_metadata_and_duplicate_never_delivers_again() {
    let sandbox = Sandbox::new("json-committed");
    sandbox.stub_channel("hermes", &format!(
        "cat >'{}'\nprintf '%s:%s\\n' \"$PNS_PRODUCER\" \"$PNS_REQUEST_ID\" >>'{}'\nprintf 'child output\\n'",
        sandbox.path("received").display(), sandbox.path("calls").display(),
    ));
    let request = request();
    let input = request.encode().unwrap();
    let first = invoke(&sandbox, &input);
    let accepted = result(&first);
    assert!(first.status.success());
    assert_eq!(accepted.status, Status::Accepted);
    assert_eq!(accepted.request_id, Some(request.request_id.clone()));
    assert!(
        accepted
            .diagnostics
            .iter()
            .any(|code| code == "ledger_committed")
    );
    assert_eq!(
        accepted.interaction,
        Some(pns_protocol::InteractionResult::NoOpinion)
    );
    assert!(stderr(&first).contains("child output"));
    let delivered: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.path("received")).unwrap()).unwrap();
    assert_eq!(delivered["agent"], "posture");
    assert_eq!(delivered["state"], "observation");
    assert_eq!(delivered["detail"], "original detail");
    assert_eq!(delivered["branch"], "original branch");
    assert_eq!(delivered["project"], "original project");
    let retained: String = database(&sandbox).query_row(
        "SELECT producer_request FROM ledger_events WHERE producer='posture' AND request_id='source-123'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(retained, input);
    let duplicate = result(&invoke(&sandbox, &input));
    assert_eq!(duplicate.status, Status::Accepted);
    assert_eq!(duplicate.decision_id, accepted.decision_id);
    assert!(
        duplicate
            .diagnostics
            .iter()
            .any(|code| code == "ledger_committed")
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("calls")).unwrap(),
        "posture:source-123\n"
    );
}

#[test]
fn json_storage_failure_attempts_live_but_never_claims_committed_ownership() {
    let sandbox = Sandbox::new("json-storage-gap");
    std::fs::write(sandbox.state(), "not a directory").unwrap();
    let output = invoke(&sandbox, &request().encode().unwrap());
    let reply = result(&output);
    assert_eq!(reply.status, Status::Degraded);
    assert!(
        !reply
            .diagnostics
            .iter()
            .any(|code| code == "ledger_committed")
    );
    assert_eq!(reply.decision_id, None);
    assert!(
        sandbox.path("hermes.event").exists(),
        "live channel was attempted"
    );
    assert_eq!(reply.request_id, Some(request().request_id));
}

#[test]
fn canonical_request_overflow_is_correlated_and_refused_before_effects() {
    let sandbox = Sandbox::new("json-canonical-overflow");
    let mut value = serde_json::json!({
        "schema":"pns.request/1", "request_id":"source-123", "producer":"posture",
        "event":"page", "signal":{"kind":"observation"},
        "extensions":{"a":"x".repeat(8000),"b":"x".repeat(8000),"c":"x".repeat(8000),
            "d":"","e":"x".repeat(8000),"f":"x".repeat(8000),"g":"x".repeat(8000),
            "h":"x".repeat(8000),"i":"x".repeat(8000)}
    });
    let room = pns_protocol::MAX_BYTES - serde_json::to_string(&value).unwrap().len();
    value["extensions"]["d"] = serde_json::json!("x".repeat(room));
    let input = serde_json::to_string(&value).unwrap();
    let decoded = pns_protocol::decode_request(input.as_bytes()).unwrap();
    assert!(
        decoded.request.encode().is_err(),
        "defaults expand the compact request beyond its cap"
    );
    let reply = result(&invoke(&sandbox, &input));
    assert_eq!(reply.status, Status::Rejected);
    assert_eq!(reply.request_id, Some(request().request_id));
    assert!(
        !sandbox.state().exists(),
        "refusal happened before storage or probes"
    );
    assert!(!sandbox.path("hermes.event").exists());
}

#[test]
fn a_json_return_keeps_replay_child_output_out_of_the_result_stream() {
    let sandbox = Sandbox::new("json-replay-output");
    pns_application::Journal::journal(
        &pns_adapters::SqliteStore::for_records(sandbox.state()),
        &pns_domain::EventArgs {
            agent: "earlier".into(),
            state: "failed".into(),
            ..Default::default()
        },
        Some(100),
        None,
    );
    sandbox.stub_channel(
        "macos-banner",
        &format!(
            "cat >'{}'\nprintf 'replay child output\\n'",
            sandbox.path("replay-event").display(),
        ),
    );
    let mut request = request();
    request.signal = Signal::Succeeded;
    request.context.pane = Some("t1:p2".into());
    let mut command = sandbox.pns_stateful();
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    let output = invoke_command(&sandbox, command, &request.encode().unwrap());
    assert_eq!(result(&output).status, Status::Accepted);
    assert!(stderr(&output).contains("replay child output"));
    let replay: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.path("replay-event")).unwrap())
            .unwrap();
    assert_eq!(replay["state"], "missed");
    assert_eq!(
        database(&sandbox)
            .query_row("SELECT count(*) FROM journal", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[path = "submit_json/observation.rs"]
mod observation;
