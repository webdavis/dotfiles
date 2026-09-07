mod fixtures {
    use crate::*;

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
    }
    /// A process id nothing is using: a child run to completion and reaped, so
    /// the kernel has already answered for it. STATED BY THE MACHINE rather
    /// than guessed at, because a made-up number can be live.
    pub(crate) fn a_reaped_pid() -> u32 {
        let mut child = std::process::Command::new("/usr/bin/true")
            .spawn()
            .expect("a child");
        let gone = child.id();
        child.wait().expect("the child is waitable");
        gone
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
}

pub(crate) use fixtures::*;
