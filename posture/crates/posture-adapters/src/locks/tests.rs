use super::*;
use crate::test_sandbox::Sandbox;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn lock_setup_creates_the_same_deployed_sibling_and_blocks_a_second_writer() {
    let sandbox = Sandbox::new("lock");
    let root = sandbox.path();
    let deployed = root.join("nested/allowlist");
    let first = AllowlistWriteLock::new(&deployed).acquire().unwrap();
    assert!(root.join("nested/allowlist.lock").is_file());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let acquired = AllowlistWriteLock::new(&deployed).acquire();
        sent.send(acquired.is_ok()).unwrap();
    });
    assert_eq!(
        received.recv_timeout(Duration::from_millis(20)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    drop(first);
    assert_eq!(received.recv_timeout(Duration::from_millis(200)), Ok(true));
}
#[test]
fn lock_parent_and_lock_file_setup_errors_both_fail_closed() {
    let sandbox = Sandbox::new("lock");
    let root = sandbox.path();
    fs::write(root.join("blocked"), b"file").unwrap();
    assert!(
        AllowlistWriteLock::new(&root.join("blocked/allowlist"))
            .acquire()
            .is_err()
    );
    fs::create_dir(root.join("allowlist.lock")).unwrap();
    assert!(
        AllowlistWriteLock::new(&root.join("allowlist"))
            .acquire()
            .is_err()
    );
}
#[test]
fn a_recorded_write_names_the_caller_the_time_the_verb_and_the_label() {
    let sandbox = Sandbox::new("lock");
    let root = sandbox.path();
    let deployed = root.join("allowlist");
    let guard = AllowlistWriteLock::new(&deployed).acquire().unwrap();
    guard.record("allow", "my.alpha").unwrap();
    guard.record("deny", "my.alpha").unwrap();
    let audit = fs::read_to_string(root.join("allowlist.audit")).unwrap();
    let lines: Vec<serde_json::Value> = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["verb"], "allow");
    assert_eq!(lines[1]["verb"], "deny");
    for line in &lines {
        assert_eq!(line["label"], "my.alpha");
        assert_eq!(line["uid"], crate::current_uid());
        assert_eq!(line["user"], serde_json::json!(crate::current_user_name()));
        let time = line["time"].as_str().unwrap();
        assert_eq!(time.len(), 20, "{time} is not an RFC 3339 UTC instant");
        assert!(time.ends_with('Z'));
    }
}
#[test]
fn a_write_whose_record_cannot_be_appended_is_refused() {
    let sandbox = Sandbox::new("lock");
    let root = sandbox.path();
    let deployed = root.join("allowlist");
    let guard = AllowlistWriteLock::new(&deployed).acquire().unwrap();
    fs::create_dir(root.join("allowlist.audit")).unwrap();
    assert_eq!(guard.record("allow", "my.alpha"), Err(RecordRefusal));
}
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn an_exec_child_cannot_keep_the_write_lock_after_the_writer_releases_it() {
    let sandbox = Sandbox::new("lock");
    let root = sandbox.path();
    let path = root.join("allowlist");
    let guard = AllowlistWriteLock::new(&path).acquire().unwrap();
    let mut child = Owned(
        Command::new("/bin/sh")
            .args(["-c", "printf ready; read -r token"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut output = child.0.stdout.take().unwrap();
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = [0; 5];
        let result = output.read_exact(&mut bytes);
        let _ = sent.send((result.is_ok(), bytes));
    });
    // FIXTURE PATIENCE, not a measurement: this waits for a spawned shell to
    // print five bytes, and nothing about the lock is being timed. It was
    // 200 ms, which a loaded machine can spend on the spawn alone.
    assert_eq!(
        received.recv_timeout(Duration::from_secs(10)),
        Ok((true, *b"ready"))
    );
    drop(guard);
    let second = OpenOptions::new()
        .write(true)
        .open(root.join("allowlist.lock"))
        .unwrap();
    // POLLED, NOT ASKED ONCE, and the property is unchanged: the exec child
    // must not KEEP this lock, so the lock must become available. One try
    // demanded that it be available in a particular instant, which is a
    // different and untrue claim in a test binary where other tests are
    // forking. `flock` belongs to the open file description, which a fork
    // shares, so any concurrent spawn holds a copy of every open descriptor
    // between its fork and its exec, close-on-exec included: CLOEXEC acts at
    // the exec, not at the fork. Measured at 3 failures in 12 runs beside the
    // lifecycle tests, whose scripts pin CPUs in infinite loops and stretch
    // that window. A child that really kept the lock blocks on `read` for the
    // rest of the test and never releases it, so the deadline still catches it.
    let available_by = Instant::now() + Duration::from_secs(5);
    loop {
        // This descriptor is owned by the fixture; nonblocking flock cannot hang on a leaked child fd.
        if unsafe { libc::flock(second.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            break;
        }
        assert!(
            Instant::now() < available_by,
            "child inherited the writer lock"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
