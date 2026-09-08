use super::*;
use std::{
    fs::File,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use ureq::unversioned::transport::{ConnectionDetails, Connector};

#[derive(Debug)]
struct HangingConnector;
impl Connector for HangingConnector {
    type Out = transport::ScriptedTransport;
    fn connect(
        &self,
        details: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        std::thread::park_timeout(*details.timeout.after);
        Err(ureq::Error::Timeout(details.timeout.reason))
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn transport_timeout_exits_four_without_success() {
    if std::env::var_os("LIGHTS_TIMEOUT_CHILD").is_some() {
        let settings = crate::settings::parse(
            "[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='test-secret'",
        )
        .unwrap();
        let agent = ureq::Agent::with_parts(
            config(Duration::from_millis(20)),
            HangingConnector,
            ScriptedResolver,
        );
        let controller = HueLightController::with_agent(&settings.controller, agent);
        assert_eq!(
            controller.room(&room()),
            Err(LightControlError::Unreachable {
                detail: "bridge request timed out".into()
            })
        );
        return;
    }
    let logs = std::env::temp_dir().join(format!("lights-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&logs).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("LIGHTS_TIMEOUT_CHILD", "1")
        .env("HOME", &logs)
        .env("TMPDIR", &logs)
        .env("TMP", &logs)
        .env("TEMP", &logs)
        .env("XDG_CONFIG_HOME", &logs)
        .env("XDG_DATA_HOME", &logs)
        .env("XDG_STATE_HOME", &logs)
        .env("XDG_CACHE_HOME", &logs)
        .env("XDG_RUNTIME_DIR", &logs)
        .env("CLAUDE_CONFIG_DIR", &logs)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args([
            "--exact",
            "hue::tests::timeout::transport_timeout_exits_four_without_success",
            "--nocapture",
        ])
        .stdin(Stdio::null())
        .stdout(File::create(logs.join("stdout")).unwrap())
        .stderr(File::create(logs.join("stderr")).unwrap());
    let mut child = OwnedChild(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_millis(400);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "timeout child failed; logs: {}",
                logs.display()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "transport exceeded independent harness deadline"
        );
        std::thread::yield_now();
    }
}
