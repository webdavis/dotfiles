//! The shared harness: a private HOME per test, stub channels, and the
//! engine spawned against them.
//!
//! Env NEVER goes through std::env::set_var: the test binary is threaded, so
//! a process-wide mutation would leak into whatever else is running. Every
//! variable rides on the Command instead.

#![allow(dead_code)] // each test binary uses its own subset of this harness.

use std::ffi::OsString;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

pub const ENGINE: &str = env!("CARGO_BIN_EXE_pns");
pub const CAPTURE: &str = env!("CARGO_BIN_EXE_http-capture");

/// The config every sandbox starts with: the three stub channels switched on,
/// and the mobile table naming the one backend compiled in. A test that needs
/// something else writes over it with `write_config`.
pub const STUB_CHANNELS: &str = "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                                 [plugins.hermes]\nenabled = true\n\
                                 [plugins.macos-banner]\nenabled = true\n";

/// Everything one test owns: a private HOME, its stub channels, and the
/// event files those stubs record into. Removed on drop.
pub struct Sandbox {
    pub root: PathBuf,
}

impl Sandbox {
    /// Named for its test, so parallel tests cannot collide and a failure
    /// leaves an identifiable directory behind.
    ///
    /// IT ARRIVES WITH A CONFIG, `STUB_CHANNELS`, because a machine with NO
    /// config runs the CORE alone (`registry::CORE`) and a test that wrote
    /// nothing would be measuring that fallback rather than the routing it was
    /// written for. It is the shape the template ships: every stub channel
    /// enabled, and the mobile table naming its backend. `without_config` is
    /// how the absence itself is tested.
    pub fn new(name: &str) -> Self {
        let sandbox = Sandbox::without_config(name);
        sandbox.write_config(STUB_CHANNELS);
        sandbox
    }

    /// A sandbox with NO config file, for the one question that is about the
    /// absence of one.
    pub fn without_config(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("channels")).expect("sandbox");
        let sandbox = Sandbox { root };
        for channel in ["mobile", "hermes", "macos-banner"] {
            sandbox.stub_channel(
                channel,
                &format!("cat >\"{}/{channel}.event\"", sandbox.display()),
            );
        }
        sandbox
    }

    /// The engine's state directory for this test, INSIDE the sandbox.
    ///
    /// Named here rather than left to `$HOME/.local/state/pns` so the daemon
    /// guard has something to assert against: a supervised loop that outlived
    /// a test and ticked over the developer's own state directory would be
    /// invisible and would keep firing.
    pub fn state(&self) -> PathBuf {
        self.path("state")
    }

    /// The engine pointed at the stubs, with its state directory pinned inside
    /// this sandbox.
    pub fn pns_stateful(&self) -> Command {
        let mut command = self.pns();
        command.env("PNS_STATE_DIR", self.state());
        command
    }

    pub fn display(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// A recording channel, or any other body: the same shape the bats stubs
    /// wrote, one script per channel name.
    pub fn stub_channel(&self, channel: &str, body: &str) {
        write_script(
            &self.root.join("channels").join(format!("{channel}.sh")),
            body,
        );
    }

    /// The engine, pointed at the stubs and away from the desk.
    pub fn pns(&self) -> Command {
        let mut command = self.bare();
        command
            .env("PNS_CHANNELS_DIR", self.root.join("channels"))
            .env("PNS_IDLE_SECS", "99999")
            // The phone's clock is read by walking the DEVELOPER'S OWN live
            // mosh sessions, so the suite states it instead: untouched for a
            // day. A test about the phone overrides this with its own age.
            .env("PNS_PHONE_INPUT_AGE", "99999")
            // No live condenser: a Stop hook spawns one for real, and the
            // suite must never reach the operator's own Codex.
            .env("CODEX_BIN", "/nonexistent/codex");
        command
    }

    /// The engine with NOTHING pointing it at stubs, which is the only way to
    /// reach the native plugins.
    ///
    /// EVERYTHING is cleared and only what the binary genuinely needs is put
    /// back, so a developer's environment cannot decide a verdict. The old
    /// blocklist named the variables to remove, which meant every new
    /// override had to be added here too or it would leak in silently; this
    /// states what a test keeps instead, and a new override is excluded by
    /// default.
    pub fn bare(&self) -> Command {
        let mut command = Command::new(ENGINE);
        command.env_clear();
        command.env("HOME", &self.root);
        // MOSHI-HOOK IS FENCED OFF BY DEFAULT, pointed at a path inside this
        // sandbox that nothing ever creates. Unset, the binary falls back to
        // `/opt/homebrew/bin/moshi-hook`, which on this machine EXISTS and is
        // the operator's own: a test that forgot to stub raised a real card on
        // a real phone during slice 11, and a second one was found by review in
        // the daemon suite. A default here makes that structural rather than
        // remembered, and every test that wants a stub still overrides it,
        // because this is set before the caller's own `env` calls.
        command.env("MOSHI_HOOK_BIN", self.root.join("no-moshi-hook-here"));
        // PATH survives because the binary resolves herdr and terminal-notifier
        // through it, and a test that stubs either one prepends to this.
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command
    }

    pub fn fired(&self, channel: &str) -> bool {
        self.path(&format!("{channel}.event")).exists()
    }

    /// The event one stub channel recorded, parsed.
    pub fn event(&self, channel: &str) -> serde_json::Value {
        let raw = std::fs::read_to_string(self.path(&format!("{channel}.event")))
            .unwrap_or_else(|_| panic!("{channel} recorded no event"));
        serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{channel}: {error}: {raw}"))
    }

    /// A stub `herdr` first on PATH, answering the two calls the session view
    /// makes. `origin_visible` decides whether the event's pane sits on the
    /// tab being looked at or on another one, which is the whole input the
    /// visibility model takes.
    ///
    /// Nothing caller-relative is answered: `pane current`, and a `pane
    /// layout` that names no pane, both resolve against whoever asked, and
    /// the view must never use either. Both exit non-zero here, so a build
    /// that regresses to one reads the session as unreadable instead of
    /// passing on a stub that could not tell the difference.
    pub fn stub_herdr(&self, command: &mut Command, origin_visible: bool) {
        let origin_tab = if origin_visible { "t1" } else { "t9" };
        self.stub_on_path(
            command,
            "herdr",
            &format!(
                r#"case "$1 $2 $3" in
  "workspace list ")      printf '%s' '{{"result":{{"workspaces":[{{"active_tab_id":"t1","focused":true,"workspace_id":"w1"}}]}}}}' ;;
  "pane layout --pane")   printf '%s' '{{"result":{{"layout":{{"focused_pane_id":"t1:p1","tab_id":"{origin_tab}","zoomed":false}}}}}}' ;;
  *)                      exit 1 ;;
esac"#
            ),
        );
    }

    /// A stub binary of that name, first on PATH.
    pub fn stub_on_path(&self, command: &mut Command, name: &str, body: &str) {
        let stub_bin = self.path("bin");
        std::fs::create_dir_all(&stub_bin).expect("stub bin");
        write_script(&stub_bin.join(name), body);
        let mut path = OsString::from(&stub_bin);
        path.push(":");
        path.push(
            command
                .get_envs()
                .find(|(key, _)| *key == "PATH")
                .and_then(|(_, value)| value)
                .map(OsString::from)
                .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
        );
        command.env("PATH", path);
    }

    /// A stub `terminal-notifier` first on PATH, so the native banner's spawn
    /// is recorded instead of posting a real notification.
    pub fn stub_notifier(&self, command: &mut Command) {
        self.stub_on_path(
            command,
            "terminal-notifier",
            &format!(
                "printf '%s\\n' \"$*\" >\"{}/notifier.args\"",
                self.display()
            ),
        );
    }

    /// The engine's config, written where its HOME will find it. The secrets
    /// live in this file now, so this is also how a native test arms a
    /// channel.
    pub fn write_config(&self, contents: &str) {
        let dir = self.path(".config/pns");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("config.toml"), contents).expect("config file");
    }

    /// The macOS Focus store with `mode` asserted, written where the engine's
    /// own HOME will find it: one live assertion, and a catalog naming that
    /// mode `name`.
    ///
    /// THE SANDBOX'S HOME IS THE WHOLE SEAM. No variable names this path, for
    /// the reason no variable can set the operator's mute: one that could
    /// would let any producer force the answer in either direction. NOTHING
    /// HERE READS THE OPERATOR'S OWN STORE, which would answer differently on
    /// every run and on every machine.
    ///
    /// The shapes are the live ones off this machine, trimmed to the keys a
    /// reader navigates; `focus.rs`'s own tests carry them at full fidelity,
    /// duplicate assertion record and all.
    pub fn write_focus_store(&self, mode: &str, name: &str) {
        let dir = self.path("Library/DoNotDisturb/DB");
        std::fs::create_dir_all(&dir).expect("focus db dir");
        std::fs::write(
            dir.join("Assertions.json"),
            format!(
                r#"{{"data":[{{"storeInvalidationRecords":[],"storeInvalidationRequestRecords":[],
                "storeAssertionRecords":[{{"assertionUUID":"3CC0682F-2B5C-4C9D-95EB-93E0B5B2677A",
                "assertionStartDateTimestamp":809713980.03135,
                "assertionDetails":{{"assertionDetailsIdentifier":"com.apple.focus.activity-manager",
                "assertionDetailsModeIdentifier":"{mode}","assertionDetailsReason":"user-action"}}}}]}}],
                "header":{{"version":8,"timestamp":809744069.273127}}}}"#
            ),
        )
        .expect("the assertion store");
        std::fs::write(
            dir.join("ModeConfigurations.json"),
            format!(
                r#"{{"data":[{{"modeConfigurations":{{"{mode}":{{"dimsLockScreen":false,
                "mode":{{"name":"{name}","identifier":"586E30E1-1C59-45D9-B531-838B7759C1E2",
                "modeIdentifier":"{mode}","visibility":0}}}}}}}}],
                "header":{{"version":3,"timestamp":809128539.244755}}}}"#
            ),
        )
        .expect("the mode catalog");
    }
}

/// A listing where the keys DISAGREE: the MAC names the phone, the client
/// name matches nobody, and the address is now the neighbouring client's
/// lease. Shared, so "stale" is one fixture rather than one per test file.
pub const KEYS_DISAGREE: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mouse","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;

/// The `[plugins.router]` table KEYS_DISAGREE is read against, and the lines
/// that reading prints. Shared, because a second test asserting the
/// diagnostic is unchanged is only worth anything if "unchanged" is the same
/// text. LAST in every config built on it, so a test can append one more
/// router setting by writing one more line.
pub fn router_table(router_url: &str) -> String {
    format!(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"{router_url}\"\n\
         device_mac = \"2e:11:ab:6d:b0:4f\"\ndevice_hostname = \"mister-2\"\n\
         device_ipv4 = \"192.168.1.248\"\napi_key = \"k-123\"\n"
    )
}

/// A UniFi router on loopback, answering the two calls the probe makes: the
/// sites listing, then whatever clients listing it has been given.
///
/// IN-PROCESS, on a thread rather than a child: the engine under test is the
/// process that has to be real, and a listener the test owns can have its
/// listing swapped between runs, which is how a resolved staleness is
/// observed. The thread ends with the test binary.
pub struct RouterStub {
    port: u16,
    listing: Arc<Mutex<String>>,
}

impl RouterStub {
    pub fn start(listing: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        let port = listener.local_addr().expect("local addr").port();
        let listing = Arc::new(Mutex::new(listing.to_string()));
        let served = Arc::clone(&listing);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &served);
            }
        });
        RouterStub { port, listing }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The same listener addressed by NAME instead of by literal address.
    ///
    /// A proxy bypass matches on HOST ALONE, port ignored, so a test that
    /// routes the gateway through a proxy and needs this stub reached
    /// directly cannot tell the two apart while both are spelled
    /// `127.0.0.1`. `localhost` resolves here and is a different host string,
    /// which is the only handle the bypass rule offers.
    pub fn localhost_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// What the router says from the next call on.
    pub fn set_listing(&self, listing: &str) {
        *self.listing.lock().expect("the listing") = listing.to_string();
    }
}

/// One request, one answer, one closed connection: `Connection: close` keeps
/// the client from pooling a socket this server has already dropped.
///
/// THE WHOLE HEADER IS READ BEFORE IT IS ROUTED, the way `http-capture` reads
/// its own request. ONE `read` is not a request: a segment boundary landing
/// before the request line, or a read that errors, leaves the routing text
/// short or empty, and text carrying no "clients" serves the SITES body as
/// the clients listing. `parse_clients` accepts that as a complete listing of
/// one anonymous client, so the test reports NotHome and reads as a flake
/// rather than as its own assertion.
fn answer(mut stream: std::net::TcpStream, listing: &Mutex<String>) {
    // A client that opens a socket and says nothing must not park this
    // thread: the accept loop is serial, so one hang would stall every later
    // request instead of failing one.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match std::io::Read::read(&mut stream, &mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let body = if String::from_utf8_lossy(&request).contains("clients") {
        listing.lock().expect("the listing").clone()
    } else {
        r#"{"data":[{"id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}]}"#.to_string()
    };
    let _ = std::io::Write::write_all(
        &mut stream,
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

/// A `pns daemon run` that is KILLED ON EVERY EXIT PATH, including a panicking
/// test.
///
/// THE SUITE'S FIRST LONG-LIVED CHILD, and the reason this is a guard rather
/// than a plain spawn. Every other long-lived thing in this tree is a thread
/// inside the test process and dies with it; a daemon left running after a
/// failed assertion keeps ticking, keeps spawning, and does it against
/// whatever state directory it was given. `Drop` runs on the panic path, so
/// the kill is not conditional on the test passing.
///
/// IT DOUBLES NOTHING. A supervised loop's behavior IS its process boundary,
/// so this drives the real binary; it is modelled on `RouterStub`, which
/// already owns a live listener for a test's lifetime.
///
/// BOTH STREAMS GO TO A FILE, which is what launchd does with this job, and
/// what lets a test read the log without racing a pipe.
pub struct DaemonGuard {
    child: std::process::Child,
    log: PathBuf,
}

impl DaemonGuard {
    /// Start the daemon against THIS sandbox, at a tick measured in
    /// milliseconds.
    ///
    /// THE STATE DIRECTORY IS ASSERTED BEFORE THE SPAWN, not documented as a
    /// convention. A tick against the operator's real `~/.local/state/pns`
    /// would run their jobs, write their heartbeat and leave their spool
    /// drained, so the one guard that must not be skippable is the one that
    /// proves this is not that directory.
    pub fn start(sandbox: &Sandbox, tick_ms: u64) -> Self {
        let state = sandbox.state();
        assert!(
            state.starts_with(&sandbox.root),
            "the daemon must tick inside the sandbox, not at {state:?}"
        );
        if let Some(home) = std::env::var_os("HOME") {
            let real = PathBuf::from(home).join(".local/state/pns");
            assert_ne!(
                state, real,
                "the daemon must never tick against the real state directory"
            );
        }
        let log = sandbox.path("daemon.log");
        let out = std::fs::File::create(&log).expect("the daemon log");
        let errors = out.try_clone().expect("the daemon log again");
        let child = sandbox
            .pns_stateful()
            .env("PNS_DAEMON_TICK_MS", tick_ms.to_string())
            .args(["daemon", "run"])
            .stdin(std::process::Stdio::null())
            .stdout(out)
            .stderr(errors)
            .spawn()
            .expect("the daemon starts");
        DaemonGuard { child, log }
    }

    /// Everything the daemon has said, both streams together.
    pub fn said(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// The status of a daemon that STOPPED ON ITS OWN, or None if it is still
    /// running when the deadline passes.
    ///
    /// POLLED WITH `try_wait` rather than `wait`, so a daemon that never exits
    /// fails the assertion instead of parking the test binary forever.
    pub fn exited_within(
        &mut self,
        deadline: std::time::Duration,
    ) -> Option<std::process::ExitStatus> {
        let end = std::time::Instant::now() + deadline;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if std::time::Instant::now() >= end => return None,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => return None,
            }
        }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn write_script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/usr/bin/env bash\n{body}\n")).expect("write script");
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod");
}

/// Run to completion, asserting the exit-0 edge: a failed notification must
/// never fail the caller.
pub fn run(command: &mut Command) -> Output {
    let output = command.output().expect("the engine runs");
    assert!(
        output.status.success(),
        "the engine must exit 0 on every path: {output:?}"
    );
    output
}

/// Poll for something a DETACHED child produces, to a deadline, and answer
/// None when it never arrived.
///
/// A FIXED SLEEP IS REFUSED. The recap runs in a process the engine spawns and
/// never waits for, so the parent exits before the child has posted; a sleep
/// long enough to be safe makes the suite slow and a short one makes it flaky,
/// and neither says what it saw. This ends on the evidence, and the CALLER
/// reports the failure, because only the caller knows how to describe what was
/// there instead.
pub fn poll_until<T>(mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(found) = probe() {
            return Some(found);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
