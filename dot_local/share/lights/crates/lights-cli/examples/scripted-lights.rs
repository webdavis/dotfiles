#[path = "../tests/support/transport.rs"]
mod transport;

use lights_adapters::HueLightController;
use serde_json::{Value, json};
use std::{path::PathBuf, process::ExitCode, sync::Arc};

fn main() -> ExitCode {
    let fixture: Value = std::env::var_os("LIGHTS_TEST_RESOURCES")
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
        .unwrap_or_else(|| {
            serde_json::from_str(include_str!("../tests/fixtures/resources.json")).unwrap()
        });
    let connector = transport::ScriptedConnector::new(vec![
        (200, fixture),
        (200, json!({"errors":[],"data":[]})),
    ]);
    let requests = Arc::clone(&connector.requests);
    let path =
        PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap()).join("lights/config.toml");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let response = if std::env::var_os("LIGHTS_TEST_TIMEOUT").is_some() {
        lights_cli::run(&args, &path, |s| {
            HueLightController::with_transport(
                s,
                transport::TimeoutConnector,
                transport::ScriptedResolver,
            )
        })
    } else {
        lights_cli::run(&args, &path, |s| {
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
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    ExitCode::from(response.exit)
}
