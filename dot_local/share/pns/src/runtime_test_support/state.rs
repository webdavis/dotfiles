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
