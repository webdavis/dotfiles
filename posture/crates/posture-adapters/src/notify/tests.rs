use super::*;

/// A home this machine does not have, so a window path can be asserted
/// without one test's state directory being another's.
const HOME: &str = "/private/fixture/home";

fn parsed(text: &str) -> NotifyMode {
    Notify::parse(text, Path::new(HOME)).expect("a usable notify choice")
}

fn copy_of(mode: &NotifyMode) -> Option<CriticalCopy> {
    match mode {
        NotifyMode::Hermes { critical_copy, .. } => critical_copy.clone(),
        other => panic!("hermes mode, not {other:?}"),
    }
}

#[test]
fn command_mode_carries_the_command_and_its_arguments_verbatim() {
    let mode = parsed(
        r#"
        [notify]
        mode = "command"
        [notify.command]
        path = "/private/fixture/engine"
        arguments = ["submit", "--json"]
        "#,
    );
    assert_eq!(
        mode,
        NotifyMode::Command {
            path: PathBuf::from("/private/fixture/engine"),
            arguments: vec!["submit".to_string(), "--json".to_string()],
        }
    );
}

#[test]
fn hermes_mode_carries_the_gateway_base_and_one_key_per_route() {
    let mode = parsed(
        r#"
        [notify]
        mode = "hermes"
        [notify.hermes]
        url = "http://127.0.0.1:8644/webhooks"
        [notify.hermes.keys]
        posture-pages = "k-posture-pages"
        priority = "k-priority"
        "#,
    );
    let NotifyMode::Hermes { base_url, keys, .. } = mode else {
        panic!("hermes mode")
    };
    assert_eq!(base_url, "http://127.0.0.1:8644/webhooks");
    assert_eq!(
        keys.get("posture-pages").map(String::as_str),
        Some("k-posture-pages")
    );
    assert_eq!(keys.get("priority").map(String::as_str), Some("k-priority"));
}

#[test]
fn off_mode_needs_no_table_of_its_own() {
    assert_eq!(parsed("[notify]\nmode = \"off\"\n"), NotifyMode::Off);
}

#[test]
fn a_mode_this_build_does_not_serve_is_refused_by_name() {
    let refusal = Notify::parse("[notify]\nmode = \"banner\"\n", Path::new(HOME)).unwrap_err();
    for named in ["banner", "hermes", "command", "off"] {
        assert!(refusal.contains(named), "{named}: {refusal}");
    }
}

#[test]
fn a_misspelled_key_or_table_blocks_the_file_rather_than_switching_notification_off() {
    for text in [
        "[notify]\nmode = \"hermes\"\nurl = \"http://x\"\n",
        "[notify]\nmode = \"command\"\n[notify.command]\npath = \"/x\"\narguents = []\n",
        "[notify]\nmode = \"off\"\n[notify.of]\n",
        "[delivery]\nmode = \"hermes\"\n",
    ] {
        assert!(Notify::parse(text, Path::new(HOME)).is_err(), "{text}");
    }
}

#[test]
fn command_mode_without_a_command_is_refused_rather_than_left_unrunnable() {
    let refusal = Notify::parse("[notify]\nmode = \"command\"\n", Path::new(HOME)).unwrap_err();
    assert!(refusal.contains("path"), "{refusal}");
}

#[test]
fn hermes_mode_states_the_local_gateway_when_the_file_names_none() {
    let mode = parsed("[notify]\nmode = \"hermes\"\n");
    assert_eq!(
        mode,
        NotifyMode::Hermes {
            base_url: DEFAULT_WEBHOOK_BASE.to_string(),
            keys: BTreeMap::new(),
            critical_copy: None,
        }
    );
}

#[test]
fn an_absent_file_leaves_the_fail_closed_default_and_no_refusal_to_report() {
    let notify = Notify::read(Path::new("/private/fixture/absent"));
    assert_eq!(notify, Notify::default());
    assert_eq!(notify.refusal, None);
    assert_eq!(
        notify.mode,
        NotifyMode::Hermes {
            base_url: DEFAULT_WEBHOOK_BASE.to_string(),
            keys: BTreeMap::new(),
            critical_copy: None,
        }
    );
}

#[test]
fn a_malformed_file_names_its_own_refusal_and_never_reads_as_an_absent_one() {
    let sandbox = crate::test_sandbox::Sandbox::new("notify-malformed");
    let home = sandbox.path();
    std::fs::create_dir_all(home.join(".config/posture")).unwrap();
    std::fs::write(config_path(home), "[notify\nmode = \"hermes\"\n").unwrap();
    let notify = Notify::read(home);
    assert!(notify.refusal.is_some());
    assert_eq!(notify.mode, Notify::default().mode);
}

#[test]
fn formatting_a_choice_names_the_routes_and_never_prints_a_signing_key() {
    let notify = Notify {
        mode: parsed(
            r#"
            [notify]
            mode = "hermes"
            [notify.hermes.keys]
            posture-pages = "s3cret-posture"
            priority = "s3cret-priority"
            "#,
        ),
        refusal: None,
    };
    let formatted = format!("{notify:?}");
    assert!(!formatted.contains("s3cret"), "{formatted}");
    assert!(formatted.contains("posture-pages"), "{formatted}");
    assert!(formatted.contains("priority"), "{formatted}");
}

#[test]
fn a_config_path_left_by_a_broken_link_is_refused_rather_than_read_as_unconfigured() {
    let sandbox = crate::test_sandbox::Sandbox::new("notify-dangling-link");
    let home = sandbox.path();
    std::fs::create_dir_all(home.join(".config/posture")).unwrap();
    std::os::unix::fs::symlink(home.join("nowhere"), config_path(home)).unwrap();
    let notify = Notify::read(home);
    assert!(notify.refusal.is_some(), "{notify:?}");
    assert_eq!(notify.mode, Notify::default().mode);
}

#[test]
fn a_named_copy_route_is_carried_with_the_state_file_its_rolling_hour_lives_in() {
    let mode = parsed(
        r#"
        [notify]
        mode = "hermes"
        [notify.hermes]
        critical_copy_route = "explain"
        "#,
    );
    assert_eq!(
        copy_of(&mode),
        Some(CriticalCopy {
            route: Name::new("explain").unwrap(),
            // THE LITERAL PATH, not the constant: the config template's own
            // comment names this file, and an operator looking for the hour
            // reads it there.
            window: PathBuf::from(HOME).join(".local/state/posture-critical-copy-window.json"),
        })
    );
}

#[test]
fn hermes_mode_without_a_copy_route_posts_one_page_and_names_no_second_route() {
    assert_eq!(copy_of(&parsed("[notify]\nmode = \"hermes\"\n")), None);
    assert_eq!(copy_of(&Notify::default().mode), None);
}

#[test]
fn a_copy_route_no_gateway_could_serve_blocks_the_file_rather_than_being_dropped() {
    // A route name is what a URL path segment is built from, so a control
    // character in one is refused at load the way every other identifier is,
    // rather than posting the copy somewhere nobody named.
    let refusal = Notify::parse(
        "[notify]\nmode = \"hermes\"\n[notify.hermes]\ncritical_copy_route = \"ex\\u0000plain\"\n",
        Path::new(HOME),
    )
    .unwrap_err();
    assert!(!refusal.is_empty(), "{refusal}");
}

#[test]
fn formatting_a_choice_names_the_copy_route_too() {
    let notify = Notify {
        mode: parsed(
            r#"
            [notify]
            mode = "hermes"
            [notify.hermes]
            critical_copy_route = "explain"
            [notify.hermes.keys]
            explain = "s3cret-explain"
            "#,
        ),
        refusal: None,
    };
    let formatted = format!("{notify:?}");
    assert!(!formatted.contains("s3cret"), "{formatted}");
    assert!(formatted.contains("explain"), "{formatted}");
}
