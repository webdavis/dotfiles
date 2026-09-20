use super::captured_child::CapturedChild;
use super::*;
use pns_protocol::{Name, Request, RequestId, State, Status};

fn input(class: Option<&str>) -> String {
    let mut request = Request::new(
        RequestId::new("class-case").unwrap(),
        Name::new("independent-tool").unwrap(),
        State::Blocked,
    );
    request.detail = "same private detail".into();
    request.delivery_class = class.map(|name| Name::new(name).unwrap());
    request.encode().unwrap()
}

fn invoke(sandbox: &Sandbox, input: &str) -> std::process::Output {
    let mut command = sandbox.pns();
    command
        .args(["send", "--json"])
        .env("PNS_STATE_DIR", sandbox.state())
        .env("PNS_SCREEN_IDLE", "0");
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
    // THE SHARED LIVENESS BOUND: every caller asserts the class decision and
    // the hermes event, none of them the elapsed time, and the 650ms this
    // carried failed under load on a build that passes alone.
    CapturedChild::spawn(&mut command)
        .unwrap()
        .input_output_within(input.as_bytes(), HANG_LIMIT)
        .unwrap()
}

/// Arms the sandbox's config with `tables` appended, a timed mute set and the
/// Sleep Focus mode on, which is the edge every case below crosses.
fn muted(name: &str, tables: &str) -> Sandbox {
    let sandbox = Sandbox::new(name);
    let config = std::fs::read_to_string(sandbox.path(".config/pns/config.toml")).unwrap();
    sandbox.write_config(&format!(
        "{config}\n[focus]\nsilence = [\"Sleep\"]\n{tables}"
    ));
    sandbox.write_focus_store("com.apple.sleep", "Sleep");
    let quiet = pns_adapters::SqliteStore::for_records(sandbox.state());
    quiet.set_mute_expiry(Some(i64::MAX as u64)).unwrap();
    assert_eq!(quiet.mute_expiry().unwrap(), Some(i64::MAX as u64));
    sandbox
}

#[test]
fn json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes() {
    // `bypass_mute` IS THE WHOLE POLICY, read off the class's own table. The
    // fourth case is the one that says a message naming NO class reads
    // `[delivery_class.default]`: the table it never mentioned is what lets it
    // through.
    for (class, tables, allowed) in [
        (
            Some("security"),
            "[delivery_class.security]\nbypass_mute = true\n",
            true,
        ),
        (
            Some("security"),
            "[delivery_class.security]\nbypass_mute = false\n",
            false,
        ),
        (
            None,
            "[delivery_class.default]\nbypass_mute = false\n",
            false,
        ),
        (None, "[delivery_class.default]\nbypass_mute = true\n", true),
        (None, "", false),
        (
            Some("custom"),
            "[delivery_class.custom]\nbypass_mute = true\n",
            true,
        ),
    ] {
        let sandbox = muted("class-policy", tables);
        let wire = input(class);
        let output = invoke(&sandbox, &wire);
        let reply = pns_protocol::decode_result(&output.stdout).unwrap();
        assert_eq!(reply.status, Status::Accepted, "{output:?}");
        assert_eq!(
            sandbox.fired("macos-banner"),
            allowed,
            "class={class:?}, {tables}"
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
    }
}

/// A class no `[delivery_class.<name>]` table defines is REFUSED, on the exit
/// code every other bad field earns, and the word is named on both channels a
/// caller has. Delivering it as the default instead would put a page on a
/// route the operator did not choose.
#[test]
fn a_class_no_table_defines_is_refused_and_named_on_both_paths() {
    for class in ["security", "Security", "health"] {
        let sandbox = muted("undefined-class", "[delivery_class.default]\n");
        let output = invoke(&sandbox, &input(Some(class)));
        let reply = pns_protocol::decode_result(&output.stdout).unwrap();
        assert_eq!(reply.status, Status::Rejected, "{class}: {output:?}");
        assert_eq!(output.status.code(), Some(2), "{class}");
        assert_eq!(
            reply.diagnostics,
            ["unknown_delivery_class".to_string(), class.to_string()],
            "the reply must name the class it refused"
        );
        assert!(!sandbox.fired("hermes"), "{class} was delivered anyway");
        assert!(!sandbox.fired("macos-banner"), "{class} reached a banner");

        let mut argv = sandbox.pns();
        argv.args([
            "send",
            "--producer",
            "independent-tool",
            "--state",
            "blocked",
            "--detail",
            "x",
            "--delivery-class",
            class,
        ])
        .env("PNS_STATE_DIR", sandbox.state())
        .env("PNS_SCREEN_IDLE", "0");
        let output = argv.output().expect("the argv path runs");
        // NO `--require-delivery`, deliberately: the always-exit-0 contract
        // covers a gateway that refused a page, never a field pns will not
        // honour, so this refusal reaches a caller that did not ask.
        assert_eq!(output.status.code(), Some(2), "argv {class}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&format!("[delivery_class.{class}]")),
            "argv {class}: {output:?}"
        );
    }
}

/// The two fields `delivery_class` replaced. A producer still sending one is
/// told so rather than having its word quietly dropped.
#[test]
fn the_retired_class_and_kind_fields_are_refused_by_name() {
    for retired in ["class", "kind"] {
        let sandbox = Sandbox::new("retired-field");
        let wire = input(None).replace(
            "\"extensions\":{}",
            &format!("\"{retired}\":\"health\",\"extensions\":{{}}"),
        );
        let output = invoke(&sandbox, &wire);
        let reply = pns_protocol::decode_result(&output.stdout).unwrap();
        assert_eq!(reply.status, Status::Rejected, "{retired}: {output:?}");
        assert!(!sandbox.fired("hermes"), "{retired} was delivered anyway");
    }
}

/// A second submission under the same id and a DIFFERENT class is a conflict,
/// the way any changed field is: the ledger answers the original rather than
/// re-deciding it.
#[test]
fn a_resubmission_under_a_second_defined_class_is_still_a_conflict() {
    let sandbox = muted(
        "class-conflict",
        "[delivery_class.security]\nbypass_mute = true\n\
         [delivery_class.changed]\nbypass_mute = true\n",
    );
    let first = invoke(&sandbox, &input(Some("security")));
    assert_eq!(
        pns_protocol::decode_result(&first.stdout).unwrap().status,
        Status::Accepted,
        "{first:?}"
    );
    let conflicting = invoke(&sandbox, &input(Some("changed")));
    assert_eq!(
        pns_protocol::decode_result(&conflicting.stdout)
            .unwrap()
            .status,
        Status::Rejected
    );
}

#[test]
fn malformed_class_or_configuration_never_grants_a_mute_exception() {
    let sandbox = Sandbox::new("invalid-class");
    let invalid =
        input(Some("security")).replace("\"delivery_class\":\"security\"", "\"delivery_class\":3");
    let output = invoke(&sandbox, &invalid);
    let refused = pns_protocol::decode_result(&output.stdout).unwrap();
    assert_eq!(refused.status, Status::Rejected);
    assert_eq!(refused.request_id.unwrap().as_str(), "class-case");
    assert!(!sandbox.state().exists(), "class is refused before storage");
    assert!(!sandbox.fired("hermes"));

    let sandbox = muted(
        "invalid-class-config",
        "[delivery_class.security]\nbypass_mute = 3\n",
    );
    let output = invoke(&sandbox, &input(Some("security")));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bypass_mute"));
    assert!(!sandbox.fired("macos-banner"));
    assert!(!sandbox.fired("mobile"));
}
