use super::Sandbox;
use std::os::unix::process::CommandExt;
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
            // ITS OWN PROCESS GROUP, so `Drop` can take the daemon's CHILDREN
            // with it. The daemon runs the failure page as a child of its own,
            // and that child serves forever with nothing telling it its parent
            // died, so killing the daemon alone left one listener per test run
            // alive: 197 of them were counted on this machine on 2026-09-09,
            // the oldest thirteen hours old.
            .process_group(0)
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
        // THE WHOLE GROUP, not the daemon alone. `start` made this child a
        // group leader, so its own children share its group id and one signal
        // reaches all of them. The daemon is still waited on afterwards,
        // because that is what reaps it.
        if let Ok(group) = i32::try_from(self.child.id()) {
            unsafe { libc::kill(-group, libc::SIGKILL) };
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;

    /// Every live pid in one process group, the leader included.
    fn group_members(group: i32) -> Vec<i32> {
        let listed = std::process::Command::new("/bin/ps")
            .args(["-o", "pid=", "-g", &group.to_string()])
            .output()
            .expect("ps lists a process group");
        String::from_utf8_lossy(&listed.stdout)
            .split_whitespace()
            .filter_map(|pid| pid.parse().ok())
            .collect()
    }

    #[test]
    fn a_group_kill_reaches_a_child_the_leader_spawned() {
        // THE LEAK THIS EXISTS TO STOP. The daemon serves the failure page from
        // a child of its own, and that child serves forever with nothing
        // telling it its parent died. Killing the daemon's pid alone left one
        // listener behind per test run; 197 were counted on this machine on
        // 2026-09-09, the oldest thirteen hours old. A stand-in stands in for
        // the daemon here so the claim is about the SIGNAL, not about pns.
        let mut leader = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 300 & sleep 300"])
            .stdin(std::process::Stdio::null())
            .process_group(0)
            .spawn()
            .expect("the stand-in starts");
        let group = i32::try_from(leader.id()).expect("a group id");

        let mut members = Vec::new();
        for _ in 0..200 {
            members = group_members(group);
            if members.len() >= 3 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            members.len() >= 3,
            "the stand-in should be a leader plus two sleeps, saw {members:?}"
        );

        unsafe { libc::kill(-group, libc::SIGKILL) };
        let _ = leader.wait();

        let mut left = group_members(group);
        for _ in 0..200 {
            if left.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            left = group_members(group);
        }
        assert!(left.is_empty(), "these outlived the group kill: {left:?}");
    }
}
