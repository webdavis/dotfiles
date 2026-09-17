use super::*;
use posture_adapters::{CommandIo, CommandOutput};
use posture_application::InspectionFailure;
use std::{
    cell::RefCell,
    ffi::OsStr,
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(super) const HEALTHY: &str = r#"[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]"#;
pub(super) const EXPOSED: &str = r#"[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]"#;
pub(super) const QUERY_TIMEOUT: &str = "fixture query timeout";
#[derive(Default)]
pub(super) struct Effects {
    pub commands: Vec<&'static str>,
    pub requests: Vec<String>,
    pub baselines_at_submit: Vec<Option<Vec<u8>>>,
}
pub(super) struct Subject(PathBuf);
impl Subject {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "posture-poll-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let subject = Self(root);
        fs::create_dir_all(subject.controls().parent().unwrap()).unwrap();
        fs::write(subject.controls(), br#"[{"id":"vault","tier":"verify","reader":"fdesetup_status","expect":"on","description":"FileVault","remedy":"Enable FileVault"}]"#).unwrap();
        subject
    }
    pub fn state(&self) -> PathBuf {
        self.0.join("state/baseline.json")
    }
    pub fn controls(&self) -> PathBuf {
        self.0.join(".local/libexec/posture/controls.json")
    }
    pub fn legacy_controls(&self) -> PathBuf {
        self.0.join(".local/libexec/osquery/posture-controls.json")
    }
    pub fn marker(&self, suffix: &str) -> PathBuf {
        let mut path = self.state().into_os_string();
        path.push(suffix);
        path.into()
    }
    pub fn seed(&self) {
        fs::create_dir(self.0.join("state")).unwrap();
        fs::write(self.state(), br#"{"firewall":"1","gatekeeper":"1","screenlock":"1","vault":"on","vault:expect":"on"}"#).unwrap();
        fs::set_permissions(self.state(), fs::Permissions::from_mode(0o600)).unwrap();
    }
    pub fn run(
        &self,
        query: &str,
        control: &str,
        accepted: bool,
    ) -> (u8, String, Rc<RefCell<Effects>>) {
        let effects = Rc::new(RefCell::new(Effects::default()));
        let runner = |kind, output: &str| Runner {
            kind,
            output: output.into(),
            accepted,
            baseline: self.state(),
            effects: effects.clone(),
        };
        let config = Configuration {
            state: self.state(),
            notify: crate::command_notify(std::path::Path::new("/fake/engine")),
            alarm: "/fake/alarm".into(),
            ..Configuration::from_home(&self.0)
        };
        let mut error = Vec::new();
        let status = execute(
            config,
            PostureQuery::with_runner(runner("query", query), "/fake/osqueryi".into()),
            ControlProbes::with_runner(
                runner("control", control),
                42,
                self.0.join("rules"),
                self.0.join("preferences"),
            ),
            runner("submit", ""),
            runner("alarm", ""),
            Some(10000),
            &mut error,
        );
        (status, String::from_utf8(error).unwrap(), effects)
    }
}
pub(super) fn assert_captured_detail(request: &str, scenario: &str) {
    let (_, detail) = include_str!("fixtures/produced-details.tsv")
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(name, _)| *name == scenario)
        .expect("scenario has an independent Bash capture");
    assert!(
        request.contains(&format!("\"detail\":{detail}")),
        "{request}"
    );
}

struct Runner {
    kind: &'static str,
    output: String,
    accepted: bool,
    baseline: PathBuf,
    effects: Rc<RefCell<Effects>>,
}
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        self.effects.borrow_mut().commands.push(self.kind);
        match self.kind {
            "query" => {
                assert_eq!(program, Path::new("/fake/osqueryi"));
                assert_eq!(args[0], "--json");
                assert!(args[1].to_string_lossy().contains("FROM screenlock"));
                if self.output == QUERY_TIMEOUT {
                    return Err(InspectionFailure::TimedOut);
                }
                assert_eq!(
                    io,
                    CommandIo::Inspection {
                        merge_stderr: false
                    }
                );
            }
            "control" => {
                assert_eq!(program, Path::new("/usr/bin/fdesetup"));
                assert_eq!(args, [OsStr::new("status")]);
                assert_eq!(io, CommandIo::Inspection { merge_stderr: true });
            }
            "submit" => {
                assert_eq!(program, Path::new("/fake/engine"));
                assert_eq!(args, [OsStr::new("send"), OsStr::new("--json")]);
                let CommandIo::Input(input) = io else {
                    panic!("stdin request required")
                };
                let request = String::from_utf8(input.to_vec()).unwrap();
                let id = request
                    .split("\"request_id\":\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap();
                let status = if self.accepted {
                    "accepted"
                } else {
                    "rejected"
                };
                let diagnostic = if self.accepted {
                    "\"ledger_committed\""
                } else {
                    ""
                };
                let bytes = format!("{{\"schema\":\"pns.result/1\",\"request_id\":\"{id}\",\"status\":\"{status}\",\"diagnostics\":[{diagnostic}]}}\n").into_bytes();
                let mut effects = self.effects.borrow_mut();
                effects
                    .baselines_at_submit
                    .push(fs::read(&self.baseline).ok());
                effects.requests.push(request);
                return Ok(CommandOutput {
                    bytes,
                    exit: if self.accepted { 0 } else { 2 },
                });
            }
            _ => panic!("unexpected independent alarm"),
        }
        Ok(CommandOutput {
            bytes: self.output.as_bytes().to_vec(),
            exit: 0,
        })
    }
}
