mod support;

use support::Fixture;

const NVIM: &str =
    r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"},{"name":"nvim"}]}}}"#;

#[test]
fn invalid_arguments_print_usage_without_invoking_herdr() {
    for args in [
        vec![],
        vec![""],
        vec!["sideways"],
        vec!["--help"],
        vec!["LEFT"],
    ] {
        let result = Fixture::new(&args).run();
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(result.stdout, "");
        assert_eq!(
            result.stderr,
            "herdr-smart-nav: usage: herdr-smart-nav left|down|up|right\n"
        );
        assert!(result.calls.is_empty());
    }
}

#[test]
fn no_pane_focuses_current_without_probing_and_ignores_trailing_arguments() {
    let result = Fixture::new(&["down", "ignored", "--help"]).run();
    assert!(result.status.success());
    assert_eq!(
        result.calls,
        [vec!["pane", "focus", "--direction", "down", "--current"]]
    );
    assert_eq!(result.stdout, "action output");
    assert_eq!(result.stderr, "action diagnostic");
}

#[test]
fn nvim_receives_each_chord_on_the_plugin_pane() {
    for (direction, chord) in [
        ("left", "ctrl+h"),
        ("down", "ctrl+j"),
        ("up", "ctrl+k"),
        ("right", "ctrl+l"),
    ] {
        let mut fixture = Fixture::new(&[direction]);
        fixture
            .command
            .env("HERDR_PANE_ID", "pane ;' \n雪")
            .env("HERDR_ACTIVE_PANE_ID", "wrong-pane")
            .env("TEST_REPLY", NVIM);
        let result = fixture.run();
        assert!(result.status.success());
        assert_eq!(
            result.calls,
            [
                vec!["pane", "process-info", "--pane", "pane ;' \n雪"],
                vec!["pane", "send-keys", "pane ;' \n雪", chord],
            ]
        );
        assert_eq!(result.stdout, "action output");
        assert_eq!(result.stderr, "action diagnostic");
    }
}

#[test]
fn empty_plugin_pane_falls_back_to_active_pane() {
    let mut fixture = Fixture::new(&["up"]);
    fixture
        .command
        .env("HERDR_PANE_ID", "")
        .env("HERDR_ACTIVE_PANE_ID", "active");
    let result = fixture.run();
    assert!(result.status.success());
    assert_eq!(
        result.calls,
        [
            vec!["pane", "process-info", "--pane", "active"],
            vec!["pane", "focus", "--direction", "up", "--pane", "active"],
        ]
    );
}

#[test]
fn malformed_or_non_nvim_process_info_falls_back_to_pane_focus() {
    for reply in [
        "not json",
        "{}",
        "{}\n{}\n",
        r#"{"result":{"process_info":{"foreground_processes":[{"name":"/bin/nvim"},{"name":"NVIM"},{}]}}}"#,
        r#"{"result":{"process_info":{"foreground_processes":{}}}}"#,
        r#"{"error":{"message":"unavailable"}}"#,
    ] {
        let mut fixture = Fixture::new(&["right"]);
        fixture
            .command
            .env("HERDR_PANE_ID", "p")
            .env("TEST_REPLY", reply);
        let result = fixture.run();
        assert!(result.status.success());
        assert_eq!(
            result.calls,
            [
                vec!["pane", "process-info", "--pane", "p"],
                vec!["pane", "focus", "--direction", "right", "--pane", "p"],
            ]
        );
    }
}

#[test]
fn probe_exit_status_does_not_discard_valid_process_info() {
    let mut fixture = Fixture::new(&["left"]);
    fixture
        .command
        .env("HERDR_PANE_ID", "p")
        .env("TEST_REPLY", NVIM)
        .env("TEST_PROBE_STATUS", "17");
    let result = fixture.run();
    assert!(result.status.success());
    assert_eq!(
        result.calls.last().unwrap(),
        &["pane", "send-keys", "p", "ctrl+h"]
    );
    assert_eq!(result.stderr, "action diagnostic");
}

#[test]
fn failed_action_preserves_output_and_success_exit() {
    let mut fixture = Fixture::new(&["left"]);
    fixture.command.env("TEST_ACTION_STATUS", "19");
    let result = fixture.run();
    assert!(result.status.success());
    assert_eq!(result.stdout, "action output");
    assert_eq!(result.stderr, "action diagnostic");
}

#[test]
fn missing_herdr_is_silent_success() {
    let mut fixture = Fixture::new(&["left"]);
    fixture
        .command
        .env_remove("HERDR_BIN_PATH")
        .env("HERDR_PANE_ID", "p");
    let result = fixture.run();
    assert!(result.status.success());
    assert!(result.calls.is_empty());
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
}
