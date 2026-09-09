use super::support::transport::{ScriptedConnector, ScriptedResolver};
use super::*;
use lights_adapters::{HueLightController, PnsNotifier};
use std::{
    cell::Cell,
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitStatus},
    sync::Arc,
};

fn notify(
    args: &[&str],
    settings: &str,
    replies: Vec<(u16, serde_json::Value)>,
    runner: impl Fn(&mut Command) -> io::Result<ExitStatus>,
) -> (lights_cli::Response, usize) {
    let root = home();
    let path = root.join("notify.toml");
    std::fs::write(&path, settings).unwrap();
    let connector = ScriptedConnector::new(replies);
    let writes = Arc::clone(&connector.requests);
    let calls = Cell::new(0);
    let notifier = PnsNotifier::with_runner(&root, |command: &mut Command| {
        assert_eq!(
            writes.lock().unwrap().len(),
            2,
            "write must precede notification"
        );
        calls.set(calls.get() + 1);
        runner(command)
    });
    let response = lights_cli::run(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        &path,
        &notifier,
        |settings| HueLightController::with_transport(settings, connector, ScriptedResolver),
    );
    (response, calls.get())
}
fn replies() -> Vec<(u16, serde_json::Value)> {
    vec![(200, fixture()), (200, json!({"errors":[],"data":[]}))]
}
fn ok(_: &mut Command) -> io::Result<ExitStatus> {
    Ok(ExitStatus::from_raw(0))
}
fn original(response: &lights_cli::Response) {
    assert_eq!(response.exit, 0);
    assert_eq!(response.stdout, "3F - Studio: off\n");
    assert!(response.stderr.is_empty());
}

#[test]
fn notification_follows_validated_write_once() {
    for args in [["toggle", "--notify"], ["--notify", "toggle"]] {
        let (response, calls) = notify(&args, config(), replies(), ok);
        original(&response);
        assert_eq!(calls, 1);
    }
}
#[test]
fn write_errors_never_notify() {
    for code in [400, 503] {
        let (response, calls) = notify(
            &["toggle", "--notify"],
            config(),
            vec![(200, fixture()), (code, json!({"errors":[],"data":[]}))],
            ok,
        );
        failure(&response, 4, &code.to_string());
        assert_eq!(calls, 0);
    }
}
#[test]
fn successful_status_with_errors_never_notifies() {
    let (response, calls) = notify(
        &["toggle", "--notify"],
        config(),
        vec![
            (200, fixture()),
            (
                200,
                json!({"errors":[{"description":"owned refusal"}],"data":[]}),
            ),
        ],
        ok,
    );
    assert_eq!(response.exit, 4);
    assert!(response.stdout.is_empty());
    assert_eq!(calls, 0);
}
#[test]
fn status_never_notifies_even_when_requested() {
    let (response, calls) = notify(
        &["status", "--notify"],
        &format!("notify=true\n{}", config()),
        vec![(200, fixture())],
        ok,
    );
    assert_eq!(response.exit, 0);
    assert_eq!(
        response.stdout,
        "3F - Studio: ON | brightness: 42.75% | scene: Read\n"
    );
    assert_eq!(calls, 0);
}
#[test]
fn notify_defaults_off() {
    for (setting, expected) in [("", 0), ("notify=false\n", 0), ("notify=true\n", 1)] {
        let (response, calls) = notify(
            &["toggle"],
            &format!("{setting}{}", config()),
            replies(),
            ok,
        );
        original(&response);
        assert_eq!(calls, expected);
    }
    for invalid in ["notify=1\n", "notify='true'\n"] {
        assert!(lights_adapters::settings::parse(&format!("{invalid}{}", config())).is_err());
    }
}
#[test]
fn missing_pns_does_not_fail_action() {
    let root = home();
    let path = root.join("config.toml");
    std::fs::write(&path, config()).unwrap();
    let notifier = PnsNotifier::new(&root);
    let response = lights_cli::run(
        &["toggle".into(), "--notify".into()],
        &path,
        &notifier,
        |settings| {
            HueLightController::with_transport(
                settings,
                ScriptedConnector::new(replies()),
                ScriptedResolver,
            )
        },
    );
    original(&response);
    assert!(!root.join(".local/libexec/pns/pns").exists());
}
#[test]
fn notification_status_never_changes_success() {
    for raw in [0, 125 << 8, 126 << 8, 127 << 8, 137 << 8, 42 << 8, 9] {
        let (response, calls) = notify(&["toggle", "--notify"], config(), replies(), |_| {
            Ok(ExitStatus::from_raw(raw))
        });
        original(&response);
        assert_eq!(calls, 1);
    }
    for kind in [io::ErrorKind::NotFound, io::ErrorKind::Other] {
        let (response, calls) = notify(&["toggle", "--notify"], config(), replies(), |_| {
            Err(io::Error::from(kind))
        });
        original(&response);
        assert_eq!(calls, 1);
    }
}
