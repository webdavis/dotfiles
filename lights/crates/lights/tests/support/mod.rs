pub(super) mod transport;

use lights::{Response, run};
use lights_adapters::HueLightController;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use transport::{ScriptedConnector, ScriptedResolver};

/// A temporary test home directory, removed when dropped.
pub struct Home(PathBuf);

impl Home {
    // A pid IS NOT UNIQUE OVER TIME: macOS recycles them, so a name built from
    // the pid and a counter can match a directory a past run left behind.
    // Clearing first is what makes the name safe to reuse; the `Drop` below
    // is what stops them piling up in the first place.
    pub fn fresh(path: PathBuf) -> Home {
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Home(path)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub fn home() -> Home {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lights-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Home::fresh(path)
}
#[test]
fn fresh_clears_pre_existing_contents() {
    let path = std::env::temp_dir().join(format!("lights-home-fresh-test-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("config.toml"), "stale").unwrap();
    let home = Home::fresh(path);
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
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
    let home = home();
    let path = home.path().join("config.toml");
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
    let home = home();
    let path = home.path().join("config.toml");
    std::fs::write(&path, config()).unwrap();
    run(&["toggle".into()], &path, &Quiet, |settings| {
        HueLightController::with_transport(
            settings,
            transport::TimeoutConnector,
            transport::ScriptedResolver,
        )
    })
}
