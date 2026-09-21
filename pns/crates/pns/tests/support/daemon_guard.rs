use super::Sandbox;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

/// A `pns gateway run` that is KILLED ON EVERY EXIT PATH, including a panicking
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
            .env("PNS_DAEMON_TICK_INTERVAL", format!("{tick_ms}ms"))
            .args(["gateway", "run"])
            .stdin(std::process::Stdio::null())
            .stdout(out)
            .stderr(errors)
            // Jobs create separate groups. Drop collects them before killing
            // this leader, while their parent still identifies our ownership.
            .process_group(0)
            .spawn()
            .expect("the daemon starts");
        DaemonGuard { child, log }
    }

    /// The daemon's own process ID, for a test that signals it or asks what
    /// it has spawned.
    pub fn pid(&self) -> i32 {
        i32::try_from(self.child.id()).expect("an owned process ID")
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
        if !matches!(self.child.try_wait(), Ok(None)) {
            return;
        }
        let pid = i32::try_from(self.child.id()).expect("an owned process ID");
        // Stop spawning and reaping before listing children. An exited job
        // stays unreaped, so its ID cannot be reused while we signal its group.
        // SAFETY: pid names our live, unreaped Child, never a caller's process.
        unsafe { libc::kill(pid, libc::SIGSTOP) };
        let mut status = 0;
        loop {
            // SAFETY: status is writable and waitpid targets only our Child.
            let waited = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) };
            if waited == pid {
                if !libc::WIFSTOPPED(status) {
                    return; // waitpid reaped a daemon that exited before stopping.
                }
                break;
            }
            if std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return;
            }
        }
        let jobs = std::process::Command::new("/usr/bin/pgrep")
            .args(["-P", &pid.to_string()])
            .output()
            .and_then(|listed| match listed.status.code() {
                Some(0 | 1) => String::from_utf8_lossy(&listed.stdout)
                    .split_whitespace()
                    .map(|job| job.parse::<i32>().map_err(std::io::Error::other))
                    .collect::<std::io::Result<Vec<_>>>(),
                _ => Err(std::io::Error::other(
                    "pgrep could not list the daemon's children",
                )),
            });
        if let Ok(jobs) = &jobs {
            for &job in jobs.iter().filter(|&&job| job > 1) {
                // SAFETY: the stopped daemon pins each child ID. spawn_job
                // gives it this group; a child still before setpgid also
                // receives the direct signal. Delivery guardians close their
                // own groups when the killed job's owner pipes close.
                unsafe {
                    libc::kill(-job, libc::SIGKILL);
                    libc::kill(job, libc::SIGKILL);
                }
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Err(error) = jobs {
            if std::thread::panicking() {
                eprintln!("daemon fixture cleanup failed: {error}");
            } else {
                panic!("daemon fixture cleanup failed: {error}");
            }
        }
    }
}
