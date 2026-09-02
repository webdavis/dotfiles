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
    // THE DETAIL, not only the generic prefix: the underlying io::Error's
    // own text is "stream did not contain valid UTF-8", and a build that
    // reports the same generic prefix for every read failure would still
    // pass without this.
    assert!(
        pty.transcript.contains("valid UTF-8"),
        "the UTF-8 detail was not carried into the refusal: {:?}",
        pty.transcript
    );
    assert!(
        !pty.transcript
            .contains("the answers ended before the walk did"),
        "a read failure was reported as the input ending: {:?}",
        pty.transcript
    );
}

#[test]
fn a_secret_typed_into_setup_never_reaches_the_pty_output() {
    // EVERY BRANCH THAT ASKS FOR A SECRET IS ARMED HERE, each with its own
    // unique value: a test that only walked the token (as this one used to)
    // cannot tell `armed_secret` from `armed` on the hermes, hue or router
    // branch, since either one composes a config that still looks right.
    const TOKEN: &str = "do-not-echo-this-token";
    const HERMES_KEY: &str = "do-not-echo-this-hermes-key";
    const HUE_KEY: &str = "do-not-echo-this-hue-key";
    const ROUTER_KEY: &str = "do-not-echo-this-router-key";
    let secrets = [TOKEN, HERMES_KEY, HUE_KEY, ROUTER_KEY];

    let sandbox = Sandbox::without_config("setup-hidden-secrets");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    // EACH ANSWER IS WRITTEN ONLY AFTER ITS OWN PROMPT IS VISIBLE: arming the
    // hidden read discards whatever is already queued (`TCSAFLUSH`), so
    // typing ahead of a prompt this walk has not printed yet would be lost.
    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");
    // ECHO IS ALREADY OFF BY THE TIME THE PROMPT IS VISIBLE: the guard arms
    // before the prompt prints, so a secret typed the instant the prompt
    // appears cannot land in a still-echoing queue.
    assert_eq!(
        pty.tcgetattr().c_lflag & libc::ECHO,
        0,
        "the secret prompt was visible while echo was still on: {:?}",
        pty.transcript
    );
    pty.write_all(format!("{TOKEN}\n").as_bytes());

    pty.read_until("Post every event to hermes", PTY_DEADLINE)
        .expect("the hermes question");
    pty.write_all(b"y\n");
    pty.read_until("the signing key that route verifies: ", PTY_DEADLINE)
        .expect("the hermes key prompt");
    pty.write_all(format!("{HERMES_KEY}\n").as_bytes());

    pty.read_until("Flash hue lights", PTY_DEADLINE)
        .expect("the hue question");
    pty.write_all(b"y\n");
    pty.read_until("the hue bridge's address on the network: ", PTY_DEADLINE)
        .expect("the hue bridge prompt");
    pty.write_all(b"10.0.0.5\n");
    pty.read_until("an API key the bridge issued: ", PTY_DEADLINE)
        .expect("the hue key prompt");
    pty.write_all(format!("{HUE_KEY}\n").as_bytes());
    pty.read_until("the rooms to flash, comma separated", PTY_DEADLINE)
        .expect("the hue rooms prompt");
    pty.write_all(b"Kitchen,Office\n");

    pty.read_until("home wifi", PTY_DEADLINE)
        .expect("the router question");
    pty.write_all(b"y\n");
    pty.read_until("Which router backend?", PTY_DEADLINE)
        .expect("the router backend prompt");
    pty.write_all(b"unifi\n");
    pty.read_until("the router's URL: ", PTY_DEADLINE)
        .expect("the router URL prompt");
    pty.write_all(b"http://192.168.1.1\n");
    pty.read_until("an API key the router issued: ", PTY_DEADLINE)
        .expect("the router key prompt");
    pty.write_all(format!("{ROUTER_KEY}\n").as_bytes());
    pty.read_until("the phone's hostname on that router: ", PTY_DEADLINE)
        .expect("the router hostname prompt");
    pty.write_all(b"my-phone\n");

    pty.read_until("macOS Focus", PTY_DEADLINE)
        .expect("the focus question");
    pty.write_all(b"y\n");
    pty.read_until("which Focus modes mean it, comma separated: ", PTY_DEADLINE)
        .expect("the focus modes prompt");
    pty.write_all(b"Work,Sleep\n");

    pty.read_until("approval left unanswered", PTY_DEADLINE)
        .expect("the nag question");
    pty.write_all(b"y\n");

    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");
    assert_eq!(status.code(), Some(0), "transcript: {:?}", pty.transcript);

    for secret in secrets {
        assert!(
            !pty.transcript.contains(secret),
            "a secret reached the pty output: {secret:?}: {:?}",
            pty.transcript
        );
    }
    // THE ECHOED ANSWER, not a bare `y`: the preamble already carries that
    // letter, so only the prompt's own tail followed by the typed answer and
    // the driver's echo of its Enter says echo was back on for an ordinary
    // question.
    assert!(
        pty.transcript.contains("[y/N]: y\r\n"),
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
    for secret in secrets {
        assert!(
            contents.contains(secret),
            "a secret did not reach the file: {secret:?}: {contents}"
        );
    }
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

#[test]
fn a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops() {
    // KILL, THEN OBSERVE, rather than a pty-level tty-stop test: SIGINT is
    // not a tty-stop signal and Rust empties a spawned child's own mask
    // before exec, so the mask this observes is the wizard's own rather
    // than something inherited. `ps -o sigmask` reads 0 for a blocked
    // process on macOS, so a live process cannot be asked directly; sending
    // a real SIGINT and watching when it lands is the only external read
    // left. SIGTTIN is NOT covered here: a pending tty-stop signal is
    // discarded rather than delivered once the process group is orphaned,
    // and this harness (like CI) starts as its own session leader, so a
    // `waitpid(WUNTRACED)` on it would hang to the deadline instead of
    // observing anything. It is reviewed, not pinned.
    use std::os::unix::process::ExitStatusExt;

    let sandbox = Sandbox::without_config("setup-signal-pending");
    let mut pty = Pty::open();
    let mut child = pty.spawn(sandbox.bare().args(["setup"]));

    pty.read_until("or press enter to pair later: ", PTY_DEADLINE)
        .expect("the first prompt");

    // THE GUARD IS ALREADY ARMED HERE, so a correct build holds this rather
    // than acting on it immediately.
    let sent = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(sent, 0, "kill: {}", std::io::Error::last_os_error());
    // NO POLL, NO SLEEP: on a correct build the signal is blocked, so the
    // child is deterministically still alive the instant after `kill`
    // returns, rather than something that has to be waited out.
    assert!(
        matches!(child.try_wait(), Ok(None)),
        "the child died from a signal the guard should still be holding"
    );

    pty.write_all(b"do-not-echo-this-token\n");
    pty.read_to_eof(PTY_DEADLINE).expect("the wizard exits");
    let status = child.wait().expect("the wizard is reaped");

    // THE HELD SIGNAL LANDS ONCE THE GUARD DROPS: `Drop` restores the
    // terminal before it unblocks the mask, so the pending SIGINT is
    // delivered only after echo is already back on, and it is what ends
    // the process rather than a normal exit.
    assert_eq!(
        status.signal(),
        Some(libc::SIGINT),
        "the held SIGINT was not delivered once the guard dropped: {:?}",
        pty.transcript
    );
    let after_exit = pty.tcgetattr();
    assert_ne!(
        after_exit.c_lflag & libc::ECHO,
        0,
        "the terminal was not restored before the pending signal was delivered"
    );
}

#[test]
fn a_dangling_symlink_at_the_config_path_is_refused_before_the_first_question() {
    // NO PTY NEEDED: the config check runs before the tty check, so a plain
    // pipe (here, `/dev/null`) is enough to tell which one fired first.
    let sandbox = Sandbox::without_config("setup-dangling-symlink");
    let config_dir = sandbox.root.join(".config/pns");
    std::fs::create_dir_all(&config_dir).expect("the config directory");
    let config_path = config_dir.join("config.toml");
    std::os::unix::fs::symlink(config_dir.join("nowhere.toml"), &config_path)
        .expect("the dangling symlink");

    let output = sandbox
        .bare()
        .args(["setup"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the wizard runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists"),
        "the dangling symlink was not caught by the pre-check: {stderr}"
    );
    assert!(
        !stderr.contains("not a terminal"),
        "the pre-check ran after the tty check instead of before it: {stderr}"
    );
}

#[test]
fn an_unreadable_config_directory_is_refused_by_path_and_cause() {
    // ROOT READS THROUGH ANY MODE, so this trick cannot produce the
    // permission error the precheck is being asked to name.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: running as root, which bypasses directory permissions");
        return;
    }
    let sandbox = Sandbox::without_config("setup-unreadable-config-dir");
    let config_dir = sandbox.root.join(".config/pns");
    std::fs::create_dir_all(&config_dir).expect("the config directory");
    std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o000))
        .expect("lock the directory down");

    let output = sandbox
        .bare()
        .args(["setup"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the wizard runs");
    // RESTORED BEFORE ANY ASSERTION CAN PANIC PAST IT: the sandbox's own
    // Drop has to walk this directory to remove it.
    std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700))
        .expect("restore the directory so the sandbox can be cleaned up");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not be checked"),
        "an unreadable directory was not refused by its own cause: {stderr}"
    );
    assert!(
        stderr.contains(&config_dir.join("config.toml").display().to_string()),
        "the refusal does not name the config path: {stderr}"
    );
}

#[test]
fn an_empty_home_is_refused_by_name_before_anything_is_written() {
    // BOTH SHAPES A LAUNCHD-LESS, MISCONFIGURED SHELL CAN HAND A PROCESS:
    // set-but-empty and absent are different environments, and a build that
    // catches only one (the empty check with `unwrap_or_default` in place of
    // `.ok()`, say) still passed with only one case here.
    for home_is_absent in [false, true] {
        let sandbox = Sandbox::without_config(if home_is_absent {
            "setup-home-absent"
        } else {
            "setup-empty-home"
        });
        let mut command = sandbox.bare();
        if home_is_absent {
            command.env_remove("HOME");
        } else {
            // `bare()` points HOME at the sandbox; this overrides it back to
            // empty.
            command.env("HOME", "");
        }
        let output = command
            // KEEPS A STILL-UNFIXED RUN'S RELATIVE `.config/pns/config.toml`
            // write inside the sandbox rather than wherever this test binary
            // happens to run from.
            .current_dir(&sandbox.root)
            .args(["setup"])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("the wizard runs");

        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("HOME"),
            "the refusal does not name HOME: {stderr}"
        );
        assert!(
            !sandbox.root.join(".config").exists(),
            "something was written under {} HOME: {stderr}",
            if home_is_absent {
                "an absent"
            } else {
                "an empty"
            }
        );
    }
}
