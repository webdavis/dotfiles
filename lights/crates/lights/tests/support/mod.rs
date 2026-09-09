pub(super) mod transport;

use lights::{Response, run};
use lights_adapters::HueLightController;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use transport::{ScriptedConnector, ScriptedResolver};

pub fn home() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lights-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
pub fn config() -> &'static str {
    "[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='test-secret'\n"
}
pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/resources.json")).unwrap()
}
pub struct Quiet;
impl lights_application::Notifier for Quiet {
    fn announce(&self, _: &lights_domain::Action) {}
}

pub fn command(
    args: &[&str],
    config: Option<&str>,
    responses: Vec<(u16, Value)>,
) -> (Response, Vec<Vec<u8>>) {
    let path = home().join("config.toml");
    if let Some(config) = config {
        std::fs::write(&path, config).unwrap();
    }
    let connector = ScriptedConnector::new(responses);
    let requests = Arc::clone(&connector.requests);
    let response = run(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        &path,
        &Quiet,
        |settings| HueLightController::with_transport(settings, connector, ScriptedResolver),
    );
    let captured = requests.lock().unwrap().clone();
    (response, captured)
}
pub fn accepted(args: &[&str]) -> (Response, Vec<Vec<u8>>) {
    command(
        args,
        Some(config()),
        vec![(200, fixture()), (200, json!({"errors":[],"data":[]}))],
    )
}
pub fn body(request: &[u8]) -> Value {
    let offset = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    serde_json::from_slice(&request[offset..]).unwrap()
}
pub fn failure(response: &Response, exit: u8, name: &str) {
    assert_eq!(response.exit, exit);
    assert!(response.stdout.is_empty());
    assert!(response.stderr.starts_with("lights: "));
    assert!(response.stderr.contains(name));
    assert_eq!(response.stderr.lines().count(), 1);
    assert!(!response.stderr.contains("test-secret"));
}
pub fn timeout_command() -> Response {
    let path = home().join("config.toml");
    std::fs::write(&path, config()).unwrap();
    run(&["toggle".into()], &path, &Quiet, |settings| {
        HueLightController::with_transport(
            settings,
            transport::TimeoutConnector,
            transport::ScriptedResolver,
        )
    })
}
