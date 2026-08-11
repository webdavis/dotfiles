//! The shared harness: a private HOME per test, stub channels, and the
//! engine spawned against them.
//!
//! Env NEVER goes through std::env::set_var: the test binary is threaded, so
//! a process-wide mutation would leak into whatever else is running. Every
//! variable rides on the Command instead.

#![allow(dead_code)] // each test binary uses its own subset of this harness.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const ENGINE: &str = env!("CARGO_BIN_EXE_pns");
pub const CAPTURE: &str = env!("CARGO_BIN_EXE_http-capture");

/// Everything one test owns: a private HOME, its stub channels, and the
/// event files those stubs record into. Removed on drop.
pub struct Sandbox {
    pub root: PathBuf,
}

impl Sandbox {
    /// Named for its test, so parallel tests cannot collide and a failure
    /// leaves an identifiable directory behind.
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("channels")).expect("sandbox");
        let sandbox = Sandbox { root };
        for channel in ["moshi", "hermes", "macos-banner"] {
            sandbox.stub_channel(
                channel,
                &format!("cat >\"{}/{channel}.event\"", sandbox.display()),
            );
        }
        sandbox
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
    pub fn relay(&self) -> Command {
        let mut command = self.bare();
        command
            .env("PNS_CHANNELS_DIR", self.root.join("channels"))
            .env("RELAY_IDLE_SECS", "99999");
        command
    }

    /// The engine with NOTHING pointing it at stubs, which is the only way to
    /// reach the native plugins. Every inherited override and proxy is
    /// cleared, so a developer's environment cannot decide a verdict.
    pub fn bare(&self) -> Command {
        let mut command = Command::new(ENGINE);
        command.env("HOME", &self.root);
        for inherited in [
            "PNS_CHANNELS_DIR",
            "PNS_TERMINAL_BUNDLE_ID",
            "PNS_PHONE_MARKER_FILE",
            "RELAY_IDLE_SECS",
            "RELAY_DESK_IDLE_SECS",
            "RELAY_SKIP_PHONE",
            "RELAY_FORCE_PHONE",
            "RELAY_PHONE_ATTENTION",
            "RELAY_MOSHI_VIEWING",
            "RELAY_HERDR_FOCUSED_PANE",
            "RELAY_AUTH_FILE",
            "RELAY_MOSHI_URL",
            "RELAY_HERMES_URL",
            "http_proxy",
            "https_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
        ] {
            command.env_remove(inherited);
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

    /// A stub `terminal-notifier` first on PATH, so the native banner's spawn
    /// is recorded instead of posting a real notification.
    pub fn stub_notifier(&self, command: &mut Command) {
        let stub_bin = self.path("bin");
        std::fs::create_dir_all(&stub_bin).expect("stub bin");
        write_script(
            &stub_bin.join("terminal-notifier"),
            &format!(
                "printf '%s\\n' \"$*\" >\"{}/notifier.args\"",
                self.display()
            ),
        );
        let mut path = OsString::from(stub_bin);
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        command.env("PATH", path);
    }

    pub fn write_auth(&self, contents: &str) -> PathBuf {
        let path = self.path("auth.json");
        std::fs::write(&path, contents).expect("auth file");
        path
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

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
