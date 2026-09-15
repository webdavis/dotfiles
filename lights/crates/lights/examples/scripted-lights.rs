#[path = "../tests/support/transport.rs"]
mod transport;

use lights_adapters::{HueLightController, PnsNotifier};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Command, ExitCode, ExitStatus},
    sync::Arc,
};

fn main() -> ExitCode {
    let fixture: Value = std::env::var_os("LIGHTS_TEST_RESOURCES")
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
        .unwrap_or_else(|| {
            serde_json::from_str(include_str!("../tests/fixtures/resources.json")).unwrap()
        });
    // One bulk read, then acceptances for as many writes as a whole-house run
    // sends. A queued response nothing asks for is never popped.
    let mut responses = vec![(200, fixture)];
    responses.extend((0..8).map(|_| (200, json!({"errors":[],"data":[]}))));
    let connector = transport::ScriptedConnector::new(responses);
    let requests = Arc::clone(&connector.requests);
    let path =
        PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap()).join("lights/config.toml");
    let state =
        PathBuf::from(std::env::var_os("XDG_STATE_HOME").unwrap()).join("lights/position.toml");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let notifications = RefCell::new(Vec::new());
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let notifier = PnsNotifier::with_runner(&home, |command: &mut Command| {
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        notifications.borrow_mut().push(argv);
        Ok(ExitStatus::from_raw(0))
    });
    let response = if std::env::var_os("LIGHTS_TEST_TIMEOUT").is_some() {
        lights::run(&args, &path, &state, &notifier, |s| {
            HueLightController::with_transport(
                s,
                transport::TimeoutConnector,
                transport::ScriptedResolver,
            )
        })
    } else {
        lights::run(&args, &path, &state, &notifier, |s| {
            HueLightController::with_transport(s, connector, transport::ScriptedResolver)
        })
    };
    let captured = requests
        .lock()
        .unwrap()
        .iter()
        .map(|bytes| String::from_utf8(bytes.clone()).unwrap())
        .collect::<Vec<_>>();
    std::fs::write(
        std::env::var_os("LIGHTS_TEST_CAPTURE").unwrap(),
        serde_json::to_vec(&captured).unwrap(),
    )
    .unwrap();
    if let Some(path) = std::env::var_os("LIGHTS_TEST_NOTIFICATIONS") {
        std::fs::write(path, serde_json::to_vec(&*notifications.borrow()).unwrap()).unwrap();
    }
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    ExitCode::from(response.exit)
}
