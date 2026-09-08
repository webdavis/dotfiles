use super::Sandbox;
use std::path::PathBuf;

/// A `pns daemon run` that is KILLED ON EVERY EXIT PATH, including a panicking
/// test.
///
/// THE SUITE'S FIRST LONG-LIVED CHILD, and the reason this is a guard rather
/// than a plain spawn. Every other long-lived thing in this tree is a thread
/// inside the test process and dies with it; a daemon left running after a
/// failed assertion keeps ticking, keeps spawning, and does it against
/// whatever state directory it was given. `Drop` runs on the panic path, so
/// the kill is not conditional on the test passing.
///
/// IT DOUBLES NOTHING. A supervised loop's behavior IS its process boundary,
/// so this drives the real binary; it is modelled on `RouterStub`, which
/// already owns a live listener for a test's lifetime.
///
/// BOTH STREAMS GO TO A FILE, which is what launchd does with this job, and
/// what lets a test read the log without racing a pipe.
pub struct DaemonGuard {
    child: std::process::Child,
    log: PathBuf,
}

impl DaemonGuard {
    /// Start the daemon against THIS sandbox, at a tick measured in
    /// milliseconds.
    ///
    /// THE STATE DIRECTORY IS ASSERTED BEFORE THE SPAWN, not documented as a
    /// convention. A tick against the operator's real `~/.local/state/pns`
    /// would run their jobs, write their heartbeat and leave their spool
    /// drained, so the one guard that must not be skippable is the one that
    /// proves this is not that directory.
    pub fn start(sandbox: &Sandbox, tick_ms: u64) -> Self {
        let state = sandbox.state();
        assert!(
            state.starts_with(&sandbox.root),
            "the daemon must tick inside the sandbox, not at {state:?}"
        );
        if let Some(home) = std::env::var_os("HOME") {
            let real = PathBuf::from(home).join(".local/state/pns");
            assert_ne!(
                state, real,
                "the daemon must never tick against the real state directory"
            );
        }
        let log = sandbox.path("daemon.log");
        let out = std::fs::File::create(&log).expect("the daemon log");
        let errors = out.try_clone().expect("the daemon log again");
        let child = sandbox
            .pns_stateful()
            .env("PNS_DAEMON_TICK_MS", tick_ms.to_string())
            .args(["daemon", "run"])
            .stdin(std::process::Stdio::null())
            .stdout(out)
            .stderr(errors)
            .spawn()
            .expect("the daemon starts");
        DaemonGuard { child, log }
    }

    /// Everything the daemon has said, both streams together.
    pub fn said(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// The status of a daemon that STOPPED ON ITS OWN, or None if it is still
    /// running when the deadline passes.
    ///
    /// POLLED WITH `try_wait` rather than `wait`, so a daemon that never exits
    /// fails the assertion instead of parking the test binary forever.
    pub fn exited_within(
        &mut self,
        deadline: std::time::Duration,
    ) -> Option<std::process::ExitStatus> {
        let end = std::time::Instant::now() + deadline;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if std::time::Instant::now() >= end => return None,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => return None,
            }
        }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
