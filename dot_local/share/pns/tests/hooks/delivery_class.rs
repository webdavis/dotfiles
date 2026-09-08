use super::captured_child::CapturedChild;
use super::*;
use pns_protocol::{Name, Request, RequestId, Signal, Status};

fn input(class: Option<&str>) -> String {
    let mut request = Request::new(
        RequestId::new("class-case").unwrap(),
        Name::new("independent-tool").unwrap(),
        Name::new("page").unwrap(),
        Signal::NeedsAttention,
    );
    request.detail = "same private detail".into();
    request.class = class.map(|name| Name::new(name).unwrap());
    request.encode().unwrap()
}

fn invoke(sandbox: &Sandbox, input: &str) -> std::process::Output {
    let mut command = sandbox.pns();
    command
        .args(["submit", "--json"])
        .env("PNS_STATE_DIR", sandbox.state())
        .env("PNS_IDLE_SECS", "0");
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
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    CapturedChild::spawn(&mut command)
        .unwrap()
        .input_output_within(input.as_bytes(), std::time::Duration::from_millis(650))
        .unwrap()
}

#[test]
fn json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes() {
    for (class, table, allowed) in [
        (Some("security"), "", true),
        (None, "", false),
        (Some("custom"), "", false),
        (Some("Security"), "", false),
        (
            Some("security"),
            "[delivery]\nbypass_silence_classes = []\n",
            false,
        ),
        (
            Some("custom"),
            "[delivery]\nbypass_silence_classes = [\"custom\"]\n",
            true,
        ),
    ] {
        let sandbox = Sandbox::new("class-policy");
        let config = std::fs::read_to_string(sandbox.path(".config/pns/config.toml")).unwrap();
        sandbox.write_config(&format!(
            "{config}\n[focus]\nsilence = [\"Sleep\"]\n{table}"
        ));
        sandbox.write_focus_store("com.apple.sleep", "Sleep");
        let quiet = pns_adapters::SqliteStore::for_records(sandbox.state());
        quiet.set_quiet_expiry(Some(i64::MAX as u64)).unwrap();
        assert_eq!(quiet.quiet_expiry().unwrap(), Some(i64::MAX as u64));
        let wire = input(class);
        let output = invoke(&sandbox, &wire);
        let reply = pns_protocol::decode_result(&output.stdout).unwrap();
        assert_eq!(reply.status, Status::Accepted, "{output:?}");
        assert_eq!(
            sandbox.fired("macos-banner"),
            allowed,
            "class={class:?}, {table}"
        );
        assert!(!sandbox.fired("mobile"), "presence still chose the desk");
        let hermes = sandbox.event("hermes");
        assert_eq!(hermes["detail"], "same private detail");
        assert_eq!(hermes["agent"], "independent-tool");
        let store = pns_adapters::SqliteStore::for_records(sandbox.state());
        let identity = pns_application::SubmissionIdentity {
            producer: "independent-tool".into(),
            request_id: "class-case".into(),
        };
        let retained = pns_application::DeliveryLedger::inspect(&store, &identity)
            .unwrap()
            .unwrap();
        assert_eq!(
            retained.submission.producer_request.as_deref(),
            Some(wire.as_str())
        );
        if class == Some("security") && allowed {
            let conflicting = invoke(&sandbox, &input(Some("changed")));
            assert_eq!(
                pns_protocol::decode_result(&conflicting.stdout)
                    .unwrap()
                    .status,
                Status::Rejected
            );
        }
    }
}

#[test]
fn malformed_class_or_configuration_never_grants_a_mute_exception() {
    let sandbox = Sandbox::new("invalid-class");
    let invalid = input(Some("security")).replace("\"class\":\"security\"", "\"class\":3");
    let output = invoke(&sandbox, &invalid);
    let refused = pns_protocol::decode_result(&output.stdout).unwrap();
    assert_eq!(refused.status, Status::Rejected);
    assert_eq!(refused.request_id.unwrap().as_str(), "class-case");
    assert!(!sandbox.state().exists(), "class is refused before storage");
    assert!(!sandbox.fired("hermes"));

    let sandbox = Sandbox::new("invalid-class-config");
    sandbox.write_config("[delivery]\nbypass_silence_classes = [3]");
    let quiet = pns_adapters::SqliteStore::for_records(sandbox.state());
    quiet.set_quiet_expiry(Some(i64::MAX as u64)).unwrap();
    assert_eq!(quiet.quiet_expiry().unwrap(), Some(i64::MAX as u64));
    let output = invoke(&sandbox, &input(Some("security")));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bypass_silence_classes"));
    assert!(!sandbox.fired("macos-banner"));
    assert!(!sandbox.fired("mobile"));
}
