use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::time::Instant;

#[test]
fn a_hanging_executable_channel_is_killed_and_reaped_even_if_it_never_reads() {
    assert_hanging_event_is_bounded("{}".to_string());
}

#[test]
fn an_executable_channel_that_never_reads_cannot_block_the_event_write() {
    assert_hanging_event_is_bounded("x".repeat(1024 * 1024));
}

/// How long the channel is left hanging before the runner is expected to kill
/// it.
///
/// FIXTURE PATIENCE, not the behavior. The script records its own process id and
/// then hangs, and everything asserted below needs that recorded id, so the
/// channel has to reach its first write before the runner kills the group. It
/// was 50 ms, which is a wall-clock budget for starting a shell while the rest
/// of the suite competes for the same CPU, and both tests using this fixture
/// failed together on a loaded machine with an unwritten id file.
const HANGS_FOR: Duration = Duration::from_millis(500);

/// The bound the delivery itself must come back inside. THIS ONE IS THE
/// MEASUREMENT: the channel sleeps for a minute, so a delivery that waits for it
/// rather than for its own deadline blows through any finite bound.
const BOUNDED_BY: Duration = Duration::from_secs(5);

fn assert_hanging_event_is_bounded(event: String) {
    let root = std::env::temp_dir().join(format!(
        "pns-executable-deadline-{}-{}",
        std::process::id(),
        event.len()
    ));
    std::fs::create_dir(&root).unwrap();
    let channel = root.join("channel.sh");
    let pid_file = root.join("pid");
    std::fs::write(
        &channel,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec /bin/sleep 60\n",
            pid_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&channel, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (send, receive) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let outcome = deliver_executable(
            &channel,
            &event,
            Some("fixture-id"),
            "fixture",
            HANGS_FOR,
            false,
        );
        send.send(outcome).unwrap();
    });
    // Patience again, not a measurement: a delivery that never returns expires
    // this however long it waits.
    let result = receive.recv_timeout(BOUNDED_BY * 2);
    let pid: libc::pid_t = std::fs::read_to_string(pid_file)
        .expect("the channel must have recorded its process id")
        .parse()
        .unwrap();
    if result.is_err() {
        // SAFETY: this fixture recorded its own unreaped direct child's ID.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    worker.join().unwrap();
    // SAFETY: signal zero observes only the fixture's recorded process ID.
    let exists = unsafe { libc::kill(pid, 0) };
    let existence_error = std::io::Error::last_os_error().raw_os_error();
    // SAFETY: WNOHANG cannot block; the runner must have consumed this wait.
    let waited = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    let wait_error = std::io::Error::last_os_error().raw_os_error();
    if waited == 0 {
        // SAFETY: waitpid confirms this is still our unreaped child.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::waitpid(pid, std::ptr::null_mut(), 0);
        }
    }
    assert!(
        result.is_ok(),
        "executable delivery exceeded the harness deadline"
    );
    assert_eq!(result.unwrap(), Delivery::Silent);
    assert_eq!(exists, -1, "the channel survived delivery");
    assert_eq!(existence_error, Some(libc::ESRCH));
    assert_eq!(waited, -1, "the runner must reap the channel");
    assert_eq!(wait_error, Some(libc::ECHILD));
    assert!(started.elapsed() < BOUNDED_BY);
}

mod egress;
