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
        assert!(
            flags >= 0,
            "fcntl F_GETFD: {}",
            std::io::Error::last_os_error()
        );
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
                // A KNOWN SIGNAL STATE, NEVER AN INHERITED ONE. Rust hands a
                // spawned child the PARENT'S OWN signal mask untouched
                // ("Inherit the signal mask from the parent rather than
                // resetting it", library/std/src/sys/process/unix/unix.rs)
                // and resets only SIGPIPE's disposition. A suite launched
                // with SIGINT blocked or ignored (a `trap '' INT` shell, a
                // job-control-less runner) would otherwise hand the wizard
                // that state, and the test that sends it a real signal would
                // then time out against a CORRECT build.
                let mut empty: libc::sigset_t = std::mem::zeroed();
                if libc::sigemptyset(&mut empty) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let masked = libc::pthread_sigmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
                if masked != 0 {
                    // POSIX: `pthread_sigmask` RETURNS its error number
                    // rather than setting errno.
                    return Err(std::io::Error::from_raw_os_error(masked));
                }
                for signal in [
                    libc::SIGINT,
                    libc::SIGALRM,
                    libc::SIGTERM,
                    libc::SIGQUIT,
                    libc::SIGHUP,
                ] {
                    if libc::signal(signal, libc::SIG_DFL) == libc::SIG_ERR {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                // NO CORE FILES FROM A TEST: SIGQUIT's default action dumps
                // one, and a suite that scatters cores across a machine
                // whose `ulimit -c` happens to be unset is a side effect
                // nobody asked this test for.
                let no_cores = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_CORE, &no_cores) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
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

#[path = "setup/input.rs"]
mod input;

#[path = "setup/hidden_input.rs"]
mod hidden_input;

#[path = "setup/paths.rs"]
mod paths;

#[path = "setup/managed_warning.rs"]
mod managed_warning;
