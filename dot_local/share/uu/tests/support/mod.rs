//! The harness every command-line test shares: one scratch HOME per test, the
//! real binary spawned against it, and the speed guard that keeps the suite
//! honest.
//!
//! WHAT THESE SUITES ARE FOR. Exit codes, the marker policy and the doctor's
//! key hygiene are decided in the binary, where no unit test reaches, and a
//! mutation there (the marker written on a failed run, a lane failure leaking
//! into the exit code, a configless machine exiting non-zero) survived the
//! whole unit suite. Each test spawns the real binary once against its own
//! scratch HOME, with the lane pointed at a stub herdr by absolute path so
//! nothing touches PATH, the network or the machine's own config.
//!
//! EVERY ITEM IS `pub` AND THE MODULE ALLOWS DEAD CODE. Cargo compiles this
//! file separately into each test binary that declares `mod support`, and each
//! of those uses a different subset, so the unused warning here is about the
//! binary being compiled rather than about the code. There is no other way to
//! share a harness across integration tests.
#![allow(dead_code)]

use std::cell::Cell;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Instant;

/// The same speed guard as pns's `Sandbox` (`dot_local/share/pns/tests/support/mod.rs`):
/// two crates, no shared dev crate, so this is a deliberate duplicate rather
/// than an import. See that file for the full reasoning behind the two
/// numbers; in short, `TEST_BUDGET_MS` is the review line (`Drop` warns on
/// stderr, greppable as "test budget", and keeps going) and `TEST_CEILING_MS`
/// is the failure line, calibrated so parallel-scheduler contention never
/// reaches it.
pub const TEST_BUDGET_MS: u128 = 1_000;
pub const TEST_CEILING_MS: u128 = 5_000;

pub fn over_budget(elapsed_ms: u128) -> bool {
    elapsed_ms > TEST_BUDGET_MS
}

pub fn over_ceiling(elapsed_ms: u128, excused: bool, panicking: bool) -> bool {
    elapsed_ms > TEST_CEILING_MS && !excused && !panicking
}

pub struct Home {
    pub dir: PathBuf,
    pub created: Instant,
    excused: Cell<bool>,
}

impl Home {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("uu-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch HOME");
        Home {
            dir,
            created: Instant::now(),
            excused: Cell::new(false),
        }
    }

    /// Excuse THIS home from `TEST_CEILING_MS` because its cost is
    /// structural rather than a regression. `&self`: tests hold their home
    /// immutably, so the excuse is a `Cell`. Never silences the WARNING at
    /// `TEST_BUDGET_MS`, only the failure at the ceiling.
    pub fn allow_slow(&self, reason: &'static str) {
        debug_assert!(!reason.is_empty(), "allow_slow needs a real reason");
        self.excused.set(true);
    }

    pub fn with_config(self, text: &str) -> Self {
        let config = self.dir.join(".config/uu");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::write(config.join("config.toml"), text).expect("config file");
        self
    }

    /// A herdr that answers every call with `exit_code`, and a lane block
    /// pointing straight at it.
    pub fn with_herdr_lane(self, exit_code: i32) -> Self {
        self.with_herdr_lane_and(&format!("exit {exit_code}\n"), "")
    }

    /// The same lane, with `body` as the stub herdr's whole script and `extra`
    /// appended to the config file.
    pub fn with_herdr_lane_and(self, body: &str, extra: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.dir.join("herdr-stub");
        std::fs::write(&stub, format!("#!/bin/sh\n{body}")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let text = format!("[lanes.herdr]\nbinary = \"{}\"\n{extra}", stub.display());
        self.with_config(&text)
    }

    /// An executable shell script written into the scratch HOME, for a
    /// command lane's `run` (or `[alerts]`'s `binary`) to point at.
    pub fn write_stub(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.dir.join(name);
        std::fs::write(&stub, format!("#!/bin/sh\n{body}")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
        stub
    }

    pub fn uu(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_uu"))
            .args(args)
            .env("HOME", &self.dir)
            .output()
            .expect("spawn uu")
    }

    pub fn marker(&self) -> PathBuf {
        self.dir.join(".local/state/uu/last-success")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let elapsed = self.created.elapsed().as_millis();
        if over_budget(elapsed) {
            // THE PROCESS'S OWN STDERR, not `eprintln!`: libtest captures the
            // print macros of a passing test and shows them only on failure or
            // under `--show-output`, which would swallow the one line the
            // review rule exists to print.
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "test budget: home {:?} took {elapsed} ms, over the {TEST_BUDGET_MS} ms budget",
                self.dir
            );
        }
        if over_ceiling(elapsed, self.excused.get(), std::thread::panicking()) {
            panic!(
                "test budget: home {:?} took {elapsed} ms, over the {TEST_CEILING_MS} ms ceiling \
                 (call allow_slow(\"reason\") if this is structural)",
                self.dir
            );
        }
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A loopback port with nothing behind it: bound only to learn a number the
/// kernel says is free, then released. A connection there is REFUSED at once,
/// so the record path's failure arm is exercised without waiting out a
/// deadline or reaching the network.
pub fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    listener.local_addr().expect("its number").port()
}

/// Accept exactly one connection, read the WHOLE request (headers plus the
/// Content-Length body), answer 200, and hand back the body as text.
///
/// Mirrors the raw-HTTP read pns's own hermes tests use
/// (`dot_local/share/pns/src/channels/hermes.rs`, the redirect test): two
/// crates, no shared dev dependency, so this is a deliberate duplicate rather
/// than an import. A response after only a partial read can reset the socket
/// under a client still writing, which is why this drains to Content-Length
/// before answering.
pub fn read_posted_body(listener: std::net::TcpListener) -> String {
    use std::io::{Read, Write};
    let (mut stream, _) = listener.accept().expect("the uu binary's POST");
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break raw.len();
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(position) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let content_length = String::from_utf8_lossy(&raw[..header_end])
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    while raw.len() < header_end + content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    let _ = stream.flush();
    String::from_utf8_lossy(&raw[header_end..header_end + content_length]).to_string()
}
