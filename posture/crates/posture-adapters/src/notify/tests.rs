use super::*;
use posture_domain::Agent;

/// A home this machine does not have, so a window path can be asserted
/// without one test's state directory being another's.
const HOME: &str = "/private/fixture/home";

fn parsed(text: &str) -> NotifyMode {
    Notify::parse(text, Path::new(HOME))
        .expect("a usable notify choice")
        .0
}

fn warnings_of(text: &str) -> Vec<String> {
    Notify::parse(text, Path::new(HOME))
        .expect("a usable notify choice")
        .1
}

/// Records what reached the local banner, so a refusal can be asserted to have
/// been raised where no config is needed to raise it.
#[derive(Default)]
struct Alarm {
    calls: Vec<(String, String)>,
}

impl posture_application::IndependentAlarm for Alarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), posture_application::AlarmFailed> {
        self.calls.push((title.into(), detail.into()));
        Ok(())
    }
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
fn a_key_this_build_does_not_read_is_named_and_never_disables_delivery() {
    // Each row is a usable file, one key it does not read, and the name that
    // key must be reported under. The delivery the file configures has to come
    // out the same either way: an unread key costs a line of warning, never a
    // destination.
    for (base, unread, named) in [
        (
            "[notify]\nmode = \"hermes\"\n[notify.hermes]\nurl = \"http://127.0.0.1:8/w\"\n",
            "gateway = \"http://127.0.0.1:9/w\"\n",
            "notify.hermes.gateway",
        ),
        (
            "[notify]\nmode = \"command\"\n[notify.command]\npath = \"/x\"\n",
            "arguents = []\n",
            "notify.command.arguents",
        ),
        // THE LIVE INSTANCE: a table a newer build of this tool writes, read
        // by an older one that has no field for it, which used to void the
        // whole file and deliver nothing at all.
        (
            "[notify]\nmode = \"command\"\n[notify.command]\npath = \"/x\"\n",
            "[jobs]\nuptime = \"com.example.x\"\n",
            "jobs.uptime",
        ),
    ] {
        let text = format!("{base}{unread}");
        let warnings = warnings_of(&text);
        assert!(
            warnings.iter().any(|warning| warning.contains(named)),
            "{named}: {warnings:?}"
        );
        assert_eq!(parsed(&text), parsed(base), "{text}");
        assert!(warnings_of(base).is_empty(), "{base}");
    }
}

#[test]
fn a_table_this_build_does_not_read_leaves_the_notify_table_beside_it_alone() {
    let mode = parsed("[delivery]\nmode = \"off\"\n[notify]\nmode = \"hermes\"\n");
    assert_eq!(mode, Notify::default().mode);
    assert!(
        warnings_of("[delivery]\nx = 1\n[notify]\nmode = \"hermes\"\n")
            .iter()
            .any(|warning| warning.contains("`delivery`")),
    );
}

#[test]
fn a_known_key_holding_a_value_it_cannot_hold_still_blocks_the_whole_file() {
    for text in [
        // A mode, a path and a route are each the one thing their own key is
        // for, so nothing can be inferred about the operator's intent from a
        // value that is not one.
        "[notify]\nmode = \"banner\"\n",
        "[notify]\nmode = 5\n",
        "[notify]\nmode = \"command\"\n[notify.command]\npath = 7\n",
        "[notify]\nmode = \"hermes\"\n[notify.hermes]\nkeys = \"one-key\"\n",
        "[notify]\nmode = \"off\"\n[jobs]\nalert = 3\n",
        // A file with no delivery choice in it at all is not a file with one
        // key too many; there is nothing to degrade to.
        "[delivery]\nmode = \"hermes\"\n",
    ] {
        assert!(Notify::parse(text, Path::new(HOME)).is_err(), "{text}");
    }
}

#[test]
fn a_config_that_will_not_parse_reaches_the_local_banner_and_not_only_a_log() {
    let notify = Notify::refused("unknown variant `banner`".to_string());
    let mut alarm = Alarm::default();
    let mut diagnostics = Vec::new();
    notify.report(&mut alarm, &mut diagnostics);
    let log = String::from_utf8(diagnostics).expect("utf-8 diagnostics");
    assert!(log.contains("no page can be delivered"), "{log}");
    let (title, detail) = alarm.calls.first().expect("a banner was raised");
    assert!(title.contains("cannot deliver"), "{title}");
    assert!(detail.contains("unknown variant `banner`"), "{detail}");
}

#[test]
fn a_usable_config_raises_no_banner_of_its_own() {
    let mut alarm = Alarm::default();
    Notify::default().report(&mut alarm, &mut Vec::new());
    assert!(alarm.calls.is_empty(), "{:?}", alarm.calls);
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
fn a_command_or_off_mode_names_no_missing_route_at_all() {
    assert_eq!(
        NotifyMode::Command {
            path: PathBuf::from("/x"),
            arguments: Vec::new(),
        }
        .missing_hermes_keys(),
        Vec::<&str>::new()
    );
    assert_eq!(NotifyMode::Off.missing_hermes_keys(), Vec::<&str>::new());
}

#[test]
fn hermes_mode_names_every_known_route_with_no_key_of_its_own() {
    let mode = parsed("[notify]\nmode = \"hermes\"\n");
    assert_eq!(
        mode.missing_hermes_keys(),
        vec!["posture-pages", "priority"]
    );
}

#[test]
fn hermes_mode_with_both_routes_keyed_names_nothing_missing() {
    let mode = parsed(
        r#"
        [notify]
        mode = "hermes"
        [notify.hermes.keys]
        posture-pages = "s3cret-posture"
        priority = "s3cret-priority"
        "#,
    );
    assert!(mode.missing_hermes_keys().is_empty(), "{mode:?}");
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
        warnings: Vec::new(),
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
        warnings: Vec::new(),
    };
    let formatted = format!("{notify:?}");
    assert!(!formatted.contains("s3cret"), "{formatted}");
    assert!(formatted.contains("explain"), "{formatted}");
}

/// The labels the `[jobs]` half of one config file states.
fn labels(text: &str) -> AgentLabels {
    toml::from_str::<schema::File>(text)
        .expect("a usable config file")
        .jobs
        .into_labels()
}

#[test]
fn a_stated_job_label_is_the_one_the_tool_uses_and_the_rest_keep_their_defaults() {
    let configured = labels(
        r#"
        [notify]
        mode = "off"
        [jobs]
        alert = "com.example.osquery-results-alerter"
        digest = "com.example.roundup"
        "#,
    );
    assert_eq!(
        configured.label(Agent::Alert),
        "com.example.osquery-results-alerter"
    );
    assert_eq!(configured.label(Agent::Digest), "com.example.roundup");
    assert_eq!(configured.label(Agent::Poll), "dev.posture.poll");
}

#[test]
fn a_file_naming_no_job_leaves_every_label_at_its_shipped_default() {
    assert_eq!(
        labels("[notify]\nmode = \"off\"\n"),
        AgentLabels::default(),
        "an absent [jobs] table is the shipped posture"
    );
    assert_eq!(
        labels("[notify]\nmode = \"off\"\n[jobs]\nalert = \"\"\n"),
        AgentLabels::default(),
        "an empty label is no label, not a job with no name"
    );
}

#[test]
fn a_job_this_build_does_not_run_leaves_every_label_it_does_run_alone() {
    let text = "[notify]\nmode = \"off\"\n[jobs]\nuptime_watchdog = \"com.example.x\"\n";
    assert_eq!(labels(text), AgentLabels::default());
    assert!(
        warnings_of(text)
            .iter()
            .any(|warning| warning.contains("jobs.uptime_watchdog")),
        "{:?}",
        warnings_of(text)
    );
}

#[test]
fn the_labels_the_watchdog_searches_for_come_off_the_config_file_on_disk() {
    let home = std::env::temp_dir().join(format!("posture-jobs-{}", std::process::id()));
    let directory = home.join(".config/posture");
    std::fs::create_dir_all(&directory).expect("a sandbox home");
    std::fs::write(
        directory.join("config.toml"),
        "[notify]\nmode = \"off\"\n[jobs]\nheartbeat = \"com.example.pulse\"\n",
    )
    .expect("a config file");
    let configured = agent_labels(&home);
    std::fs::remove_dir_all(&home).expect("the sandbox is removable");
    assert_eq!(configured.label(Agent::Heartbeat), "com.example.pulse");
}
