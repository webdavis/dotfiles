use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub struct Fixture {
    pub root: PathBuf,
}
impl Fixture {
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-shell-{}-{name}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        for leaf in [".config/pns", "channels", "bin"] {
            std::fs::create_dir_all(root.join(leaf)).unwrap();
        }
        std::fs::write(root.join(".config/pns/config.toml"), "[plugins.hermes]\nenabled = true\n[plugins.mobile]\nenabled = false\n[plugins.macos-banner]\nenabled = false\n").unwrap();
        let channel = root.join("channels/hermes.sh");
        std::fs::write(&channel, "#!/bin/sh\ncat >\"$HOME/hermes.event\"\n").unwrap();
        std::fs::set_permissions(channel, std::fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["herdr", "terminal-notifier", "ioreg", "pgrep", "ps"] {
            let path = root.join("bin").join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self { root }
    }
    pub fn marker(&self) -> PathBuf {
        self.root
            .join(format!("state/lights-shell/{}", std::process::id()))
    }
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pns"));
        cmd.env_clear()
            .env("HOME", &self.root)
            .env("PNS_STATE_DIR", self.root.join("state"))
            .env("PNS_CHANNELS_DIR", self.root.join("channels"))
            .env("PNS_IDLE_SECS", "99999")
            .env("PNS_PHONE_INPUT_AGE", "99999")
            .env("MOSHI_HOOK_BIN", self.root.join("no-moshi"))
            .env("CODEX_BIN", self.root.join("no-codex"))
            .env("PWD", "/owned/logical project")
            .env("HERDR_PANE_ID", "t1:p2")
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.join("bin").display()),
            )
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        for (key, leaf) in [
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "xdg-state"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_RUNTIME_DIR", "runtime"),
            ("XDG_CONFIG_DIRS", "config-dirs"),
            ("XDG_DATA_DIRS", "data-dirs"),
            ("CLAUDE_CONFIG_DIR", "claude"),
            ("TMPDIR", "tmp"),
            ("TMP", "tmp"),
            ("TEMP", "tmp"),
        ] {
            std::fs::create_dir_all(self.root.join(leaf)).unwrap();
            cmd.env(key, self.root.join(leaf));
        }
        cmd
    }
    pub fn run(&self, args: &[&str]) -> Output {
        let mut command = self.command();
        command
            .args(["shell", args[0], "--pid", &std::process::id().to_string()])
            .args(&args[1..]);
        capture(&mut command)
    }
    pub fn begin(&self, command: &str) {
        let output = self.run(&["begin", "--command", command]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    pub fn event(&self) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if let Ok(text) = std::fs::read_to_string(self.root.join("hermes.event"))
                && let Ok(value) = serde_json::from_str(&text)
            {
                return value;
            }
            assert!(Instant::now() < deadline, "no complete Hermes event");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

pub fn capture(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(600);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("shell command exceeded fixture deadline");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    child.wait_with_output().unwrap()
}

mod pane;
pub use pane::Pane;
