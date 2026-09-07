use super::*;

// --- observation mode: the configuration-change watch (D5) ------------------
//
// `ConfigChange` fires when Claude Code's own configuration changes underneath
// a session. Routed through `Attempt::Observation` exactly like quota and
// model-switch above: every test here plants its own precondition and asserts
// the stub channel fired INSIDE it, and every negative assertion carries a
// First-attempt control run AFTER it on the SAME sandbox, because a delivered
// card proves dispatch, not that the writer under test was reachable in that
// setup.

/// The five documented sources and the exact label this binary's own
/// allowlist (`config_source_label` in main.rs) maps each one to.
const CONFIG_CHANGE_SOURCES: [(&str, &str); 5] = [
    ("user_settings", "user settings changed"),
    ("project_settings", "project settings changed"),
    ("local_settings", "local settings changed"),
    ("policy_settings", "policy settings changed"),
    ("skills", "skills changed"),
];

pub(crate) fn config_change_payload(
    session: &str,
    source: &str,
    file_path: Option<&str>,
) -> String {
    match file_path {
        Some(path) => format!(
            r#"{{"session_id":"{session}","cwd":"/a/dotfiles","source":"{source}","file_path":"{path}"}}"#
        ),
        None => {
            format!(r#"{{"session_id":"{session}","cwd":"/a/dotfiles","source":"{source}"}}"#)
        }
    }
}

#[test]
fn each_config_change_source_delivers_one_card_naming_itself_and_its_file() {
    for (source, label) in CONFIG_CHANGE_SOURCES {
        let sandbox = Sandbox::new(&format!("config-change-card-{source}"));
        sandbox.write_config(&nag_config(300));
        counted_channels(&sandbox);

        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "config-change",
            &config_change_payload("s1", source, Some("/Users/op/.claude/settings.json")),
        );

        assert!(output.status.success(), "{source}");
        assert_eq!(deliveries(&sandbox, "hermes"), 1, "{source}");
        let event = sandbox.event("hermes");
        assert_eq!(event["state"], "config-change", "{source}");
        assert_eq!(event["agent"], "claude", "{source}");
        assert_eq!(
            event["detail"],
            format!("{label}: /Users/op/.claude/settings.json"),
            "{source}: names which source changed and the file"
        );
    }
}

#[test]
fn a_config_change_with_no_file_names_only_the_source() {
    // W3: the payload carries no key, no old or new value and no actor, so a
    // source with no `file_path` states only which source changed, never a
    // trailing colon with nothing after it.
    let sandbox = Sandbox::new("config-change-no-file");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "project_settings", None),
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1);
    let event = sandbox.event("hermes");
    assert_eq!(
        event["detail"], "project settings changed",
        "no colon and no file when the payload named none"
    );
}

#[test]
fn config_change_events_each_deliver_their_own_card_with_no_once_ever_guarantee() {
    // W2: there is no once-per-something promise to keep here. A
    // corrupt-file recovery's own intermediate write, several live sessions,
    // or a changed skill can each raise their own event, so this fires again
    // for every distinct invocation rather than coalescing repeats into one
    // card.
    let sandbox = Sandbox::new("config-change-repeats-each-card");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    for _ in 0..3 {
        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "config-change",
            &config_change_payload("s1", "user_settings", None),
        );
        assert!(output.status.success());
    }

    assert_eq!(
        deliveries(&sandbox, "hermes"),
        3,
        "three received events, three cards: no coalescing"
    );
}

#[test]
fn a_hostile_file_path_is_sanitised_before_it_reaches_the_card() {
    // W5: a right-to-left override survives `flattened` (it is Cf, not the Cc
    // `flattened` strips) and could reorder the rendered line the same way it
    // could in a model name; the config-change arm must strip it too.
    let sandbox = Sandbox::new("config-change-hostile-path");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        "{\"session_id\":\"s1\",\"source\":\"user_settings\",\"file_path\":\"/a/dotfiles/sett\u{202e}ings.json\"}",
    );

    assert!(output.status.success());
    assert_eq!(deliveries(&sandbox, "hermes"), 1);
    let event = sandbox.event("hermes");
    assert_eq!(
        event["detail"], "user settings changed: /a/dotfiles/settings.json",
        "the override character is gone from the rendered path"
    );
}

#[test]
fn an_unrecognised_config_source_delivers_nothing_and_writes_nothing() {
    // W4: THIS TEST IS VACUOUS ALONE, in `a_non_auto_model_switch_source_...`'s
    // own style: an unknown hook word exits 0 and writes nothing, so
    // "delivers nothing" would hold even with no `config-change` arm at all.
    // Prove a documented source fires FIRST, on this same sandbox, then prove
    // every shape the reference does not list leaves every trace
    // byte-identical to that snapshot: missing, empty, the wrong JSON type, a
    // different case, and a prefix of a real value, which the declaration's
    // own exact-string matcher already refuses but the Rust parser does not
    // enforce on its own.
    let sandbox = Sandbox::new("config-change-unrecognised-source-silent");
    sandbox.write_config(&nag_config(300));
    counted_channels(&sandbox);

    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "config-change",
        &config_change_payload("s1", "user_settings", None),
    );
    assert!(output.status.success(), "a documented source still exits 0");
    assert_eq!(
        deliveries(&sandbox, "hermes"),
        1,
        "a documented source delivers"
    );
    let deliveries_after = deliveries(&sandbox, "hermes");
    let decisions_after =
        std::fs::read_to_string(sandbox.path("state/decisions")).unwrap_or_default();
    let activity_after = state_lines(&sandbox, "activity");
    let present_after =
        std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default();

    let cases = [
        ("missing", r#"{"session_id":"s2"}"#.to_string()),
        ("empty", r#"{"session_id":"s2","source":""}"#.to_string()),
        ("a number", r#"{"session_id":"s2","source":7}"#.to_string()),
        (
            "wrong case",
            r#"{"session_id":"s2","source":"User_Settings"}"#.to_string(),
        ),
        (
            "a prefix of a real one",
            r#"{"session_id":"s2","source":"user_settingsx"}"#.to_string(),
        ),
        (
            "an unlisted word",
            r#"{"session_id":"s2","source":"global_settings"}"#.to_string(),
        ),
    ];
    for (case, payload) in cases {
        let output = hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "config-change",
            &payload,
        );
        assert!(output.status.success(), "{case}: still exits 0");
        assert_eq!(
            deliveries(&sandbox, "hermes"),
            deliveries_after,
            "{case}: delivers nothing"
        );
        assert_eq!(
            std::fs::read_to_string(sandbox.path("state/decisions")).unwrap_or_default(),
            decisions_after,
            "{case}: writes no decision line"
        );
        assert_eq!(
            state_lines(&sandbox, "activity"),
            activity_after,
            "{case}: writes no activity line"
        );
        assert_eq!(
            std::fs::read_to_string(sandbox.path("state/last-present")).unwrap_or_default(),
            present_after,
            "{case}: moves no presence edge"
        );
    }
}
