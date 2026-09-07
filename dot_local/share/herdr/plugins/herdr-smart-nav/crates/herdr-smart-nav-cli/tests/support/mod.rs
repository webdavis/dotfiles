use std::fs::{self, File, Permissions};
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Fixture {
    root: PathBuf,
    pub command: Command,
    started: Instant,
}

pub struct Result {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub calls: Vec<Vec<String>>,
}

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Only the process group created for this invocation belongs to this test.
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.0.wait();
    }
}

impl Fixture {
    pub fn new(args: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "smart-nav-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let binary = root.join("herdr double");
        fs::write(
            &binary,
            r#"#!/bin/bash
set -euo pipefail
count=0
if [[ -f "$TEST_ROOT/count" ]]; then read -r count < "$TEST_ROOT/count"; fi
count=$((count + 1))
printf '%s\n' "$count" > "$TEST_ROOT/count"
printf '%s\0' "$@" > "$TEST_ROOT/call-$count"
if [[ "${2-}" == process-info ]]; then
  printf '%s' "${TEST_REPLY-}"
  printf '%s' 'probe diagnostic' >&2
  exit "${TEST_PROBE_STATUS-0}"
fi
printf '%s' 'action output'
printf '%s' 'action diagnostic' >&2
exit "${TEST_ACTION_STATUS-0}"
"#,
        )
        .unwrap();
        fs::set_permissions(&binary, Permissions::from_mode(0o700)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-smart-nav"));
        command
            .args(args)
            .env_clear()
            .current_dir(&root)
            .process_group(0);
        for name in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
            "XDG_RUNTIME_DIR",
            "CLAUDE_CONFIG_DIR",
            "TMPDIR",
            "TMP",
            "TEMP",
        ] {
            let directory = root.join(name);
            fs::create_dir(&directory).unwrap();
            command.env(name, directory);
        }
        command
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("PATH", &root)
            .env("TEST_ROOT", &root)
            .env("HERDR_BIN_PATH", binary)
            .stdin(Stdio::null());
        Self {
            root,
            command,
            started: Instant::now(),
        }
    }

    pub fn run(mut self) -> Result {
        self.command
            .stdout(File::create(self.root.join("stdout")).unwrap())
            .stderr(File::create(self.root.join("stderr")).unwrap());
        let mut child = OwnedChild(self.command.spawn().unwrap());
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                self.started.elapsed() < Duration::from_millis(700),
                "plugin exceeded deadline"
            );
            std::thread::yield_now();
        };
        let calls = (1..)
            .map_while(|n| fs::read(self.root.join(format!("call-{n}"))).ok())
            .map(|bytes| {
                bytes[..bytes.len() - 1]
                    .split(|b| *b == 0)
                    .map(|arg| String::from_utf8(arg.to_vec()).unwrap())
                    .collect()
            })
            .collect();
        assert!(
            self.started.elapsed() < Duration::from_secs(1),
            "test exceeded one second"
        );
        Result {
            status,
            stdout: fs::read_to_string(self.root.join("stdout")).unwrap(),
            stderr: fs::read_to_string(self.root.join("stderr")).unwrap(),
            calls,
        }
    }
}
