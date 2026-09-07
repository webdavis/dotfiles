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
        let outcome = deliver(&channel, &event, Duration::from_millis(50));
        send.send(outcome).unwrap();
    });
    let result = receive.recv_timeout(Duration::from_millis(600));
    let pid: libc::pid_t = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
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
    assert!(started.elapsed() < Duration::from_secs(1));
}
