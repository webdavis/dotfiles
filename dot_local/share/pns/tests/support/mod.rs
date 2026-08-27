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
    fn stub_on_path(&self, command: &mut Command, name: &str, body: &str) {
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
