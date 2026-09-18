mod home;
pub(super) mod transport;

pub use home::Home;
use lights::{Response, run};
use lights_adapters::HueLightController;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use transport::{ScriptedConnector, ScriptedResolver};

pub fn home() -> Home {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lights-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Home::fresh(path)
}
pub fn config() -> &'static str {
    "[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='test-secret'\n\
certificate='sha256:0000000000000000000000000000000000000000000000000000000000000000'\n"
}
/// The line `status` adds, naming the certificate the bridge is held to. The
/// pin is the one the shared config carries, and no handshake was refused in a
/// scripted-transport test.
pub const PIN_STATE: &str = "certificate: pinned and matched \
sha256:0000000000000000000000000000000000000000000000000000000000000000\n";

pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/resources.json")).unwrap()
}
/// The same bridge after a pause: every scene reports itself inactive, which is
/// what the bridge does once a room has sat untouched.
pub fn fixture_with_no_active_scene() -> Value {
    let mut fixture = fixture();
    for resource in fixture["data"].as_array_mut().unwrap() {
        if resource["type"] == "scene" {
            resource["status"]["active"] = json!("inactive");
        }
    }
    fixture
}
pub struct Quiet;
impl lights_application::Notifier for Quiet {
    fn announce(&self, _: &lights_domain::Action) {}
    fn alarm(&self, _: &str) {}
}

pub fn command(
    args: &[&str],
    config: Option<&str>,
    responses: Vec<(u16, Value)>,
) -> (Response, Vec<Vec<u8>>) {
    command_in(&home(), args, config, responses)
}
/// The same invocation against a home that outlives it, so a second press sees
/// what the first one left on disk.
pub fn command_in(
    home: &Home,
    args: &[&str],
    config: Option<&str>,
    responses: Vec<(u16, Value)>,
) -> (Response, Vec<Vec<u8>>) {
    let path = home.path().join("config.toml");
    if let Some(config) = config {
        std::fs::write(&path, config).unwrap();
    }
    let connector = ScriptedConnector::new(responses);
    let requests = Arc::clone(&connector.requests);
    let response = run(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        &path,
        &home.path().join("state/position.toml"),
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
    let home = home();
    let path = home.path().join("config.toml");
    std::fs::write(&path, config()).unwrap();
    let state = home.path().join("state/position.toml");
    run(&["toggle".into()], &path, &state, &Quiet, |settings| {
        HueLightController::with_transport(
            settings,
            transport::TimeoutConnector,
            transport::ScriptedResolver,
        )
    })
}
