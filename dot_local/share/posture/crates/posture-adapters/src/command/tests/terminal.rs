use super::*;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::mpsc;

fn collect(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sent.send(bytes);
    });
    received
}
#[test]
fn publication_modes_keep_terminal_input_and_forward_separate_errors() {
    const MARKER: &str = "POSTURE_OWNED_TERMINAL_MODE";
    if let Ok(mode) = std::env::var(MARKER) {
        // This fixture runs in a separately owned test process. Even terminal setup is inside
        // the parent's deadline, and setsid makes that known unreaped pid own the fixture group.
        assert_ne!(unsafe { libc::setsid() }, -1);
        let (mut master_fd, mut slave_fd) = (0, 0);
        // openpty initializes two fresh descriptors, adopted exactly once below.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master_fd,
                    &mut slave_fd,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let mut master = unsafe { File::from_raw_fd(master_fd) };
        let slave = unsafe { File::from_raw_fd(slave_fd) };
        // Only the installed stdin may survive exec, never the fixture's master endpoint.
        for descriptor in [master.as_raw_fd(), slave.as_raw_fd()] {
            assert_ne!(
                unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) },
                -1
            );
        }
        // Descriptor 0 belongs to this dedicated fixture process. No live terminal is attached.
        assert_ne!(
            unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCSCTTY.into(), 0) },
            -1
        );
        assert_ne!(unsafe { libc::dup2(slave.as_raw_fd(), 0) }, -1);
        master.write_all(b"private-token\n").unwrap();
        let mut runner = SystemRunner::new(Duration::from_millis(200));
        let io = if mode == "capture" {
            CommandIo::CaptureStdout
        } else {
            CommandIo::InheritAll
        };
        let output=runner.run(Path::new("/bin/sh"), &[OsStr::new("-c"),OsStr::new("test -t 0 || exit 4; printf 'private prompt\\n' >&2; read -r answer; printf 'answer:%s' \"$answer\"")],io);
        let expected = if mode == "capture" {
            b"answer:private-token".to_vec()
        } else {
            Vec::new()
        };
        // Retain the owned master until this dedicated process exits. Closing the controlling
        // PTY earlier sends SIGHUP to this fixture itself, hiding the assertion result.
        let _master_until_process_exit = master.into_raw_fd();
        assert_eq!(
            unsafe { libc::tcgetpgrp(0) },
            unsafe { libc::getpgrp() },
            "terminal ownership was not restored"
        );
        assert_eq!(output, Ok(expected));
        return;
    }
    for mode in ["capture", "inherit"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact","command::tests::terminal::publication_modes_keep_terminal_input_and_forward_separate_errors","--nocapture"])
            .env(MARKER,mode).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let stdout = collect(child.stdout.take().unwrap());
        let stderr = collect(child.stderr.take().unwrap());
        let end = Instant::now() + Duration::from_millis(450);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= end {
                // This direct child is still unreaped, so its pid cannot have been reused.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        let out = stdout.recv_timeout(Duration::from_millis(100)).unwrap();
        let err = stderr.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            status.is_some_and(|status| status.success()),
            "{mode}: {}",
            String::from_utf8_lossy(&err)
        );
        assert!(
            err.windows(b"private prompt\n".len())
                .any(|s| s == b"private prompt\n")
        );
        if mode == "inherit" {
            assert!(
                out.windows(b"answer:private-token".len())
                    .any(|s| s == b"answer:private-token")
            );
        }
    }
}
