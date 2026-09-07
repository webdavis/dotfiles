use super::*;

fn stalled_pipe_is_reaped(closed: &str) {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", &format!("exec {closed}>&-; exec /bin/sleep 60")]);
    let capture = CapturedChild::spawn(&mut command).expect("fixture runs");
    let pid = libc::pid_t::try_from(capture.child.id()).expect("child ID");
    let result = capture.output_within(Duration::from_millis(100));
    assert_eq!(
        result.expect_err("open pipe times out").kind(),
        io::ErrorKind::TimedOut
    );
    let mut status = 0;
    // SAFETY: waitpid only queries this fixture's child ID and a valid pointer.
    let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    let error = io::Error::last_os_error().raw_os_error();
    if waited != -1 {
        // SAFETY: this failed-control fixture is still ours; reap before asserting.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::waitpid(pid, &mut status, 0);
        }
    }
    assert_eq!(waited, -1, "the capture owner must reap before returning");
    assert_eq!(error, Some(libc::ECHILD));
}

#[test]
fn stdout_timeout_reaps_the_child_before_returning() {
    stalled_pipe_is_reaped("2");
}

#[test]
fn stderr_timeout_reaps_the_child_before_returning() {
    stalled_pipe_is_reaped("1");
}

fn failure_waits_for_reader_before_returning(held: usize) {
    let child = Command::new("/bin/sleep")
        .arg("60")
        .process_group(0)
        .spawn()
        .expect("fixture runs");
    let (release_out, out) = mpsc::channel();
    let (release_err, err) = mpsc::channel();
    let capture = CapturedChild {
        child,
        stdout: mpsc::channel().1,
        stderr: mpsc::channel().1,
        readers: vec![
            std::thread::spawn(move || {
                let _ = out.recv();
            }),
            std::thread::spawn(move || {
                let _ = err.recv();
            }),
        ],
        reaped: false,
    };
    let releases = [release_out, release_err];
    releases[1 - held].send(()).expect("other reader released");
    let (finished, completion) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        let result = capture.output_within(Duration::from_millis(100));
        let _ = finished.send(result);
    });
    let before_release = completion.recv_timeout(Duration::from_millis(100));
    let _ = releases[held].send(());
    owner.join().expect("owner returns");
    assert!(matches!(
        before_release,
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(completion.recv().expect("cleanup finished").is_err());
}

#[test]
fn failure_joins_stdout_reader_before_returning() {
    failure_waits_for_reader_before_returning(0);
}

#[test]
fn failure_joins_stderr_reader_before_returning() {
    failure_waits_for_reader_before_returning(1);
}
