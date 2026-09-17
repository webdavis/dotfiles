use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The liveness bound on one `posture allowlist` run, the same bound and the
/// same reasoning as this suite's `usage.rs` and `funnel_fixture`: a hang fails
/// the row instead of wedging the suite, and NOTHING here reads the elapsed
/// time. Every case asserts the exit code, the streams and the recorded calls.
///
/// FIFTEEN SECONDS, not the 400ms this carried, which was a wall-clock budget
/// for spawning the real binary and three bash stubs while the operator's other
/// agent lanes compile. A passing run ends on the child's own exit.
const LIVENESS_BOUND: Duration = Duration::from_secs(15);

pub struct Fixture {
    pub root: PathBuf,
    pub source: PathBuf,
    pub deployed: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // The epoch nanosecond keeps a RECYCLED process id off an earlier
        // run's leftovers: nothing removes these roots, so a counter beside
        // the id alone rebuilds paths an earlier run already filled, and the
        // `create_dir` below then answers AlreadyExists. Observed on this
        // machine 2026-09-17, all six rows at once.
        let root = std::env::temp_dir().join(format!(
            "posture-curation-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for directory in ["h", "c", "d", "s", "k", "r", "e", "f", "cl", "t"] {
            fs::create_dir(root.join(directory)).unwrap();
        }
        let source = root.join("source");
        let deployed = root.join("deployed");
        fs::write(&source, b"# old source\n").unwrap();
        fs::write(&deployed, b"old deployed\n").unwrap();
        fs::write(root.join("h/agent.plist"), b"hello world").unwrap();
        fs::write(
            root.join("query.json"),
            format!(
                "[{{\"path\":\"{}/h/agent.plist\",\"program\":\"/owned program --arg\"}}]",
                root.display()
            ),
        )
        .unwrap();
        script(
            &root.join("osqueryi"),
            "printf 'query\n' >> \"$FIXTURE/calls\"\n/bin/cat \"$FIXTURE/query.json\"\n",
        );
        script(
            &root.join("chezmoi"),
            "printf '%s\n' \"$1\" >> \"$FIXTURE/calls\"\ncase \"$1\" in\nsource-path) printf '%s\n' \"$FIXTURE/source\" ;;\napply) /bin/cp \"$FIXTURE/source\" \"$OSQUERY_LAUNCHD_ALLOWLIST\"; test ! -f \"$FIXTURE/apply-fails\" ;;\n*) exit 99 ;;\nesac\n",
        );
        script(
            &root.join("manifest"),
            "printf 'manifest\n' >> \"$FIXTURE/calls\"\ntest ! -f \"$FIXTURE/manifest-fails\"\n",
        );
        Self {
            root,
            source,
            deployed,
        }
    }
    pub fn calls(&self) -> String {
        fs::read_to_string(self.root.join("calls")).unwrap_or_default()
    }
    pub fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_posture"));
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("FIXTURE", &self.root)
            .env("OSQUERY_LAUNCHD_ALLOWLIST", &self.deployed)
            .env("OSQUERYI", self.root.join("osqueryi"))
            .env("CHEZMOI", self.root.join("chezmoi"))
            .env(
                "OSQUERY_PIPELINE_MANIFEST_RUNNER",
                self.root.join("manifest"),
            )
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        for (key, directory) in [
            ("HOME", "h"),
            ("XDG_CONFIG_HOME", "c"),
            ("XDG_DATA_HOME", "d"),
            ("XDG_STATE_HOME", "s"),
            ("XDG_CACHE_HOME", "k"),
            ("XDG_RUNTIME_DIR", "r"),
            ("XDG_CONFIG_DIRS", "e"),
            ("XDG_DATA_DIRS", "f"),
            ("CLAUDE_CONFIG_DIR", "cl"),
            ("TMPDIR", "t"),
            ("TMP", "t"),
            ("TEMP", "t"),
        ] {
            command.env(key, self.root.join(directory));
        }
        let mut child = command
            .arg("allowlist")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let expires = Instant::now() + LIVENESS_BOUND;
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= expires {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned allowlist fixture never finished");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
fn script(path: &std::path::Path, body: &str) {
    fs::write(path, format!("#!/bin/bash\nset -euo pipefail\n{body}")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
