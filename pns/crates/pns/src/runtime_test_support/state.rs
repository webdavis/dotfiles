mod fixtures {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A scratch state directory of this test's own, named so two tests and two
    /// runs of one test never share a file.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "pns-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        directory
            .canonicalize()
            .expect("the canonical scratch directory")
    }
    /// A published state file's mode, which is the only thing the test below
    /// grades.
    pub(crate) fn published_mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("the published file")
            .permissions()
            .mode()
            & 0o777
    }

    pub(crate) fn in_private_process() -> bool {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let thread = std::thread::current();
        let name = thread.name().expect("the named test thread");
        if std::env::var("PNS_PRIVATE_UNIT_TEST").as_deref() == Ok(name) {
            return false;
        }
        let directory = scratch("private-unit-home");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env_clear()
            .env("HOME", &directory)
            .env("TMPDIR", &directory)
            .env("PATH", "/usr/bin:/bin")
            .env("PNS_PRIVATE_UNIT_TEST", name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // A GUARD AGAINST A HUNG CHILD, not a measurement: the child's own test
        // asserts the behavior, and a passing child ends the wait the moment it
        // exits. Loading a test binary and running one test costs well under a
        // second on an idle machine and several seconds on a loaded runner, so
        // the guard is the fixture budget the integration tests share.
        const FIXTURE_BUDGET: Duration = Duration::from_secs(30);
        let deadline = Instant::now() + FIXTURE_BUDGET;
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        true
    }
}

pub(crate) use fixtures::*;
