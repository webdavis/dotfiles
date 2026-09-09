use super::*;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

fn root() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "posture-lock-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
#[test]
fn lock_setup_creates_the_same_deployed_sibling_and_blocks_a_second_writer() {
    let root = root();
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
    let root = root();
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
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn an_exec_child_cannot_keep_the_write_lock_after_the_writer_releases_it() {
    let root = root();
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
    assert_eq!(
        received.recv_timeout(Duration::from_millis(200)),
        Ok((true, *b"ready"))
    );
    drop(guard);
    let second = OpenOptions::new()
        .write(true)
        .open(root.join("allowlist.lock"))
        .unwrap();
    // This descriptor is owned by the fixture; nonblocking flock cannot hang on a leaked child fd.
    assert_eq!(
        unsafe { libc::flock(second.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0,
        "child inherited the writer lock"
    );
}
