//! The setup wizard, end to end, through a real pty: `is_terminal()` gates
//! the whole walk, so a pipe cannot drive it and these tests give the binary
//! an actual controlling terminal the way an interactive shell would.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use support::Sandbox;

/// How long any single wait on the pty may block before a test fails BY
/// NAME instead of hanging the CI runner. Not a timing assertion: nothing
/// here measures how fast the wizard answers, only how long a read may wait
/// for output that, on a working build, arrives almost immediately.
const PTY_DEADLINE: Duration = Duration::from_secs(2);

/// A pty pair standing in for an interactive terminal, so the wizard's own
/// `is_terminal()` check sees a real one.
struct Pty {
    master: libc::c_int,
    /// `-1` once `spawn` has handed the slave to a child and closed the
    /// parent's own copy: a pty only reports EOF to the master once every
    /// descriptor naming the slave side is closed, and a fork hands this
    /// process a second one that must not outlive the child.
    slave: libc::c_int,
    transcript: String,
}

impl Pty {
    fn open() -> Pty {
        let mut master: libc::c_int = -1;
        let mut slave: libc::c_int = -1;
        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(opened, 0, "openpty: {}", std::io::Error::last_os_error());
        // THE MASTER MUST NOT SURVIVE A FORK IT DID NOT MEAN TO REACH: an
        // inherited master would let a child hold open its own controlling
        // terminal's other end, and this test's own read side would then
        // never see EOF once the wizard exits.
        let flags = unsafe { libc::fcntl(master, libc::F_GETFD) };
        assert!(flags >= 0, "fcntl F_GETFD: {}", std::io::Error::last_os_error());
        let set = unsafe { libc::fcntl(master, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        assert_eq!(set, 0, "fcntl F_SETFD: {}", std::io::Error::last_os_error());
        Pty {
            master,
            slave,
            transcript: String::new(),
        }
    }

    /// Spawn `command` with the pty's slave standing in for stdin, stdout
    /// AND stderr, the way a shell hands a program its controlling terminal
    /// on all three. The PARENT'S OWN copy of the slave is closed right
    /// after, because holding it open would keep this process's own read of
    /// the master from ever reaching EOF once the child exits.
    fn spawn(&mut self, command: &mut Command) -> Child {
        let slave = self.slave;
        unsafe {
            command.pre_exec(move || {
                for target in [0, 1, 2] {
                    if libc::dup2(slave, target) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                libc::close(slave);
                Ok(())
            });
        }
        let child = command.spawn().expect("the wizard spawns");
        unsafe { libc::close(self.slave) };
        self.slave = -1;
        child
    }

    /// One poll-then-read, bounded by `timeout`. `Ok(0)` is the pty's own
    /// EOF (macOS answers a closed slave with `EIO` rather than a `0`-byte
    /// read, so that is folded in here); `Err` names why no read could
    /// happen at all, so a hang fails by name rather than parking the test.
    fn read_once(&mut self, timeout: Duration) -> Result<usize, String> {
        let mut description = libc::pollfd {
            fd: self.master,
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let polled = unsafe { libc::poll(&mut description, 1, millis) };
        if polled < 0 {
            return Err(format!("poll: {}", std::io::Error::last_os_error()));
        }
        if polled == 0 {
            return Err("poll timed out waiting for the pty".to_string());
        }
        let mut chunk = [0u8; 4096];
        let read = unsafe { libc::read(self.master, chunk.as_mut_ptr().cast(), chunk.len()) };
        if read < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EIO) {
                return Ok(0);
            }
            return Err(format!("read: {error}"));
        }
        if read > 0 {
            self.transcript
                .push_str(&String::from_utf8_lossy(&chunk[..read as usize]));
        }
        Ok(read as usize)
    }

    /// Read until `marker` has appeared in the transcript so far, or the
    /// deadline passes. A WRONG MARKER FAILS BY NAME: the assertion is
    /// stated as a string that shows up in the panic message rather than as
    /// a hang nobody can tell apart from a slow machine.
    fn read_until(&mut self, marker: &str, deadline: Duration) -> Result<(), String> {
        let end = Instant::now() + deadline;
        while !self.transcript.contains(marker) {
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "timed out waiting for {marker:?}; transcript so far: {:?}",
                    self.transcript
                ));
            }
            if self.read_once(remaining)? == 0 {
                return Err(format!(
                    "the pty closed before {marker:?} appeared; transcript: {:?}",
                    self.transcript
                ));
            }
        }
        Ok(())
    }

    /// Read to the pty's own EOF, bounded the same way `read_until` is.
    fn read_to_eof(&mut self, deadline: Duration) -> Result<(), String> {
        let end = Instant::now() + deadline;
        loop {
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "never reached EOF; transcript so far: {:?}",
                    self.transcript
                ));
            }
            if self.read_once(remaining)? == 0 {
                return Ok(());
            }
        }
    }

    fn write_all(&self, bytes: &[u8]) {
        let mut written = 0usize;
        while written < bytes.len() {
            let result = unsafe {
                libc::write(
                    self.master,
                    bytes[written..].as_ptr().cast(),
                    bytes.len() - written,
                )
            };
            assert!(result >= 0, "write: {}", std::io::Error::last_os_error());
            written += result as usize;
        }
    }

    /// The master's OWN termios: on macOS a pty's master reflects the
    /// slave's settings and keeps reporting them after the slave side has
    /// closed, which is what lets a test check the tty state after the
    /// wizard has already exited.
    fn tcgetattr(&self) -> libc::termios {
        let mut attributes: libc::termios = unsafe { std::mem::zeroed() };
        let got = unsafe { libc::tcgetattr(self.master, &mut attributes) };
        assert_eq!(
            got,
            0,
            "tcgetattr(master): {}",
            std::io::Error::last_os_error()
        );
        attributes
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            if self.slave >= 0 {
                libc::close(self.slave);
            }
            libc::close(self.master);
        }
    }
}

#[test]
fn a_non_utf8_paste_is_reported_as_a_read_failure_rather_than_the_answers_ending() {
    let sandbox = Sandbox::without_config("setup-non-utf8");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");
    // ISTRIP IS OFF ON THIS PTY, so a byte outside plain ASCII passes
    // through unmangled to `read_line`, which is where the crate's ONLY
    // non-UTF-8 read failure lives; a bare `\xff` is not valid UTF-8 on its
    // own or paired with anything that follows it.
    pty.write_all(&[0xFF, b'\n']);

    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");
    assert_eq!(status.code(), Some(2), "transcript: {:?}", pty.transcript);
    assert!(
        pty.transcript.contains("the answers could not be read"),
        "the real reason was not reported: {:?}",
        pty.transcript
    );
    assert!(
        !pty.transcript.contains("the answers ended before the walk did"),
        "a read failure was reported as the input ending: {:?}",
        pty.transcript
    );
}

#[test]
fn a_secret_typed_into_setup_never_reaches_the_pty_output() {
    let sandbox = Sandbox::without_config("setup-hidden-secret");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    // EACH ANSWER IS WRITTEN ONLY AFTER ITS OWN PROMPT IS VISIBLE: arming the
    // hidden read discards whatever is already queued (`TCSAFLUSH`), so
    // typing ahead of a prompt this walk has not printed yet would be lost.
    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");
    pty.write_all(b"do-not-echo-this-token\n");

    pty.read_until("Post every event to hermes", PTY_DEADLINE)
        .expect("the hermes question");
    pty.write_all(b"n\n");

    pty.read_until("Flash hue lights", PTY_DEADLINE)
        .expect("the hue question");
    pty.write_all(b"n\n");

    pty.read_until("home wifi", PTY_DEADLINE)
        .expect("the router question");
    pty.write_all(b"n\n");

    pty.read_until("macOS Focus", PTY_DEADLINE)
        .expect("the focus question");
    pty.write_all(b"n\n");

    pty.read_until("approval left unanswered", PTY_DEADLINE)
        .expect("the nag question");
    pty.write_all(b"y\n");

    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");
    assert_eq!(status.code(), Some(0), "transcript: {:?}", pty.transcript);

    assert!(
        !pty.transcript.contains("do-not-echo-this-token"),
        "the secret reached the pty output: {:?}",
        pty.transcript
    );
    assert!(
        pty.transcript.contains('y'),
        "an ordinary, non-secret answer stopped echoing too: {:?}",
        pty.transcript
    );
    // ECHONL IS WHAT PRODUCES THIS, with echo itself off: the prompt's own
    // ": " is immediately followed by the driver's echo of the typed Enter,
    // and then the next question, with no echoed secret in between.
    assert!(
        pty.transcript.contains(": \r\nPost every event"),
        "the hidden prompt's Enter was not echoed via ECHONL: {:?}",
        pty.transcript
    );

    let published = sandbox.root.join(".config/pns/config.toml");
    let contents = std::fs::read_to_string(&published).expect("the published config");
    assert!(
        contents.contains("do-not-echo-this-token"),
        "the token did not reach the file: {contents}"
    );
    let mode = std::fs::metadata(&published)
        .expect("the published config")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the config is not the operator's alone");

    // THE GUARD MUST HAVE DROPPED, restoring the terminal it borrowed, even
    // though the child has already exited: a pty's master keeps reporting
    // the slave's last settings after the slave side closes.
    let after_exit = pty.tcgetattr();
    assert_ne!(
        after_exit.c_lflag & libc::ECHO,
        0,
        "echo was not restored once the wizard exited"
    );
}
