use super::*;
use crate::test_sandbox::Sandbox;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
fn directory() -> Sandbox {
    let sandbox = Sandbox::new("canary");
    fs::set_permissions(&sandbox, fs::Permissions::from_mode(0o700)).unwrap();
    sandbox
}
#[test]
fn real_file_reads_a_canary_after_more_than_one_read_buffer() {
    let dir = directory();
    let path = dir.join("snapshots.log");
    let mut bytes = b"torn\n".repeat(4000);
    bytes.extend_from_slice(b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n");
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        SnapshotsFile::new(path)
            .newest_canary()
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}
#[test]
fn absent_log_and_directory_fail_without_a_false_canary() {
    let dir = directory();
    assert_eq!(
        SnapshotsFile::new(dir.join("absent")).newest_canary(),
        Err(SnapshotReadFailure)
    );
    assert_eq!(
        SnapshotsFile::new(dir.to_path_buf()).newest_canary(),
        Err(SnapshotReadFailure)
    );
}
#[test]
fn a_fifo_log_refuses_without_waiting_for_a_writer() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let dir = directory();
    let path = dir.join("snapshots.log");
    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    // The owned private path is NUL-terminated and remains alive for mkfifo.
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(SnapshotsFile::new(path).newest_canary());
    });
    assert_eq!(
        receive
            .recv_timeout(std::time::Duration::from_millis(150))
            .expect("FIFO read must not block"),
        Err(SnapshotReadFailure)
    );
}
#[test]
fn a_read_failure_is_not_an_empty_successful_snapshot() {
    struct Broken;
    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("owned read failure"))
        }
    }
    assert_eq!(newest(BufReader::new(Broken)), Err(SnapshotReadFailure));
}
