use super::*;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

/// The liveness bound on the owned child re-exec: a hang fails the case
/// instead of wedging the suite, and NOTHING reads the elapsed time. The cases
/// assert the child's exit status and its own reaping, and the deadlines they
/// prove are the child's own 80ms and 500ms budgets.
///
/// FIFTEEN SECONDS, not the 250ms and 300ms these carried, which were
/// wall-clock budgets for starting a second copy of the test binary while the
/// operator's other agent lanes compile. A passing run ends on the child's own
/// write, or its own exit.
pub(super) const LIVENESS_BOUND: Duration = Duration::from_secs(15);

const MODE: &str = "PNS_RECAP_OWNED_FIXTURE";
const ROOT: &str = "PNS_RECAP_OWNED_ROOT";

pub(super) struct Owned {
    child: Child,
}

impl Drop for Owned {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

pub(super) struct Fixture {
    pub(super) root: PathBuf,
}
impl Fixture {
    pub(super) fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "pns-recap-{}-{label}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self { root }
    }
    pub(super) fn spawn(&self, mode: &str) -> Owned {
        let name = std::thread::current().name().unwrap().to_string();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(MODE, mode)
            .env(ROOT, &self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        Owned { child }
    }
    pub(super) fn ready(&self) {
        self.file("ready");
    }

    fn file(&self, name: &str) {
        let end = Instant::now() + LIVENESS_BOUND;
        while !self.root.join(name).exists() {
            assert!(Instant::now() < end, "fixture did not write {name}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub(super) fn wait(&self, owned: &mut Owned) -> Option<ExitStatus> {
        let child = &mut owned.child;
        let end = Instant::now() + LIVENESS_BOUND;
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return Some(status);
            }
            if Instant::now() >= end {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(super) fn child_fixture() -> bool {
    let Ok(mode) = std::env::var(MODE) else {
        return false;
    };
    let fixture = Fixture {
        root: PathBuf::from(std::env::var_os(ROOT).unwrap()),
    };

    let budget = if mode == "source-hang" { 80 } else { 500 };
    let result = recap_with_deadline(Instant::now() + Duration::from_millis(budget), || {
        std::fs::write(fixture.root.join("ready"), "reading").unwrap();
        if mode == "source-hang" {
            loop {
                std::thread::park();
            }
        }

        42
    });
    // This isolated process owns no child except its cleanup guardian.
    assert_eq!(
        unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    std::fs::write(fixture.root.join("complete"), "reaped").unwrap();
    std::process::exit(result);
}
