use super::*;
use std::{
    fs,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    process::Child,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) child: PathBuf,
    pub(super) ready: PathBuf,
}
impl Fixture {
    pub(super) fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "lights-notify-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let child = root.join("pns");
        let ready = root.join("ready");
        fs::write(&child,format!("#!/bin/bash\nset -euo pipefail\ntrap '' TERM\nprintf '%s %s\\n' \"$$\" \"$PPID\" >'{}'\nexec /bin/sleep 30\n",ready.display())).unwrap();
        fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, child, ready }
    }
}
fn exists(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}
struct Owned {
    process: Child,
    ready: PathBuf,
}
impl Drop for Owned {
    fn drop(&mut self) {
        if let Ok(ids) = fs::read_to_string(&self.ready) {
            for pid in ids.split_whitespace() {
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", pid])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
#[test]
fn pns_child_hang_is_killed_and_reaped_without_failing_action() {
    const FIXTURE: &str = "LIGHTS_OWNED_NOTIFICATION_FIXTURE";
    if let Some(root) = std::env::var_os(FIXTURE) {
        let root = PathBuf::from(root);
        let mut notifier = PnsNotifier::new(&root);
        notifier.pns = root.join("pns");
        notifier.duration = Duration::from_millis(80);
        notifier.announce(&action());
        let ids = fs::read_to_string(root.join("ready")).unwrap();
        for pid in ids.split_whitespace() {
            assert!(
                !exists(pid),
                "monitor and direct child must be gone before announce returns"
            );
        }
        fs::write(root.join("complete"), "original success").unwrap();
        return;
    }
    let fixture = Fixture::new("bounded");
    let name = std::thread::current().name().unwrap().to_owned();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &name, "--nocapture"])
        .env(FIXTURE, &fixture.root)
        .stdin(Stdio::null())
        .stdout(fs::File::create(fixture.root.join("stdout")).unwrap())
        .stderr(fs::File::create(fixture.root.join("stderr")).unwrap())
        .process_group(0);
    let mut owned = Owned {
        process: command.spawn().unwrap(),
        ready: fixture.ready.clone(),
    };
    let until = Instant::now() + Duration::from_millis(400);
    while !fixture.ready.exists() {
        assert!(Instant::now() < until, "owned pns did not become ready");
        std::thread::sleep(Duration::from_millis(1));
    }
    let status = loop {
        if let Some(status) = owned.process.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < until,
            "notification exceeded independent harness deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(
        status.success(),
        "{}",
        fs::read_to_string(fixture.root.join("stderr")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("complete")).unwrap(),
        "original success"
    );
    for pid in fs::read_to_string(&fixture.ready)
        .unwrap()
        .split_whitespace()
    {
        assert!(!exists(pid));
    }
}
