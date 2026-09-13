use super::*;
use crate::SystemRunner;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn directory() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "posture-ssh-command-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn executable(root: &Path, body: &str) -> PathBuf {
    let file = root.join("fixture-tool");
    fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
    file
}
fn runner() -> SystemRunner {
    SystemRunner::per_command(Duration::from_millis(80))
        .with_termination_grace(Duration::from_millis(20))
}

#[test]
fn every_sshd_reader_is_bounded_and_a_later_reader_gets_its_own_deadline() {
    let root = directory();
    let tool = executable(&root, "while :; do :; done");
    let mut sshd = SshdCommand::new(runner(), tool, root.join("config"), None);
    let started = Instant::now();
    for result in [
        sshd.global(),
        sshd.connection("user=fixture,host=localhost,addr=127.0.0.1"),
        sshd.syntax(),
    ] {
        assert_eq!(result, Err(InspectionFailure::TimedOut));
    }
    // Three 80 ms deadlines plus 20 ms of grace each is 300 ms of waiting. The
    // bound only has to tell that apart from a deadline applied in the wrong
    // unit or waited past, and three process spawns on a loaded runner cost
    // hundreds of milliseconds, so it is ten times the waiting rather than two.
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_key_record_on_stderr_cannot_prove_readiness() {
    let root = directory();
    let tool = executable(&root, "printf 'host key material\\n' >&2");
    let mut probe = SshKeyscan::new(runner(), tool);
    assert!(probe.available());
    assert_eq!(
        probe.probe(2222, 1),
        Ok(SshCompleted {
            status: 0,
            output: vec![]
        })
    );
}

#[test]
fn availability_matches_the_legacy_execute_predicate_including_directories() {
    let root = directory();
    assert!(runnable(&root));
    assert!(!runnable(&root.join("absent")));
    let file = root.join("not-executable");
    fs::write(&file, "").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!runnable(&file));
}

struct PrivateFiles {
    root: PathBuf,
    runner: SystemRunner,
}
impl CommandRunner for PrivateFiles {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert!(
            [
                "/usr/bin/tee",
                "/bin/chmod",
                "/bin/cp",
                "/bin/mv",
                "/bin/rm"
            ]
            .iter()
            .any(|allowed| program == Path::new(allowed))
        );
        assert!(args.iter().all(|arg| {
            let path = Path::new(arg);
            if path.is_absolute() {
                path.starts_with(&self.root)
            } else {
                matches!(arg.to_str(), Some("--" | "-Rp" | "-f" | "0644"))
            }
        }));
        self.runner.run_completed(program, args, io)
    }
}

#[test]
fn owned_file_operations_save_symlinks_publish_exact_bytes_and_restore_the_saved_type() {
    let root = directory();
    let mut files = SshFileInstaller::new(
        PrivateFiles {
            root: root.clone(),
            runner: runner(),
        },
        root.clone(),
        None,
    );
    let target = files.path(SshFile::Target);
    symlink(root.join("absent-original"), &target).unwrap();
    assert!(files.exists(SshFile::Target).unwrap());
    assert_eq!(files.prime().unwrap().status, 0);
    assert_eq!(files.stage(b"exact policy\n").unwrap().status, 0);
    assert_eq!(files.chmod().unwrap().status, 0);
    assert_eq!(files.save().unwrap().status, 0);
    assert_eq!(
        fs::read_link(files.path(SshFile::SavedTarget)).unwrap(),
        root.join("absent-original")
    );
    assert_eq!(
        files
            .rename(SshFile::Staging, SshFile::Target)
            .unwrap()
            .status,
        0
    );
    assert_eq!(fs::read(&target).unwrap(), b"exact policy\n");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o7777,
        0o644
    );
    assert_eq!(
        files
            .rename(SshFile::SavedTarget, SshFile::Target)
            .unwrap()
            .status,
        0
    );
    assert_eq!(
        fs::read_link(&target).unwrap(),
        root.join("absent-original")
    );
    assert_eq!(files.remove(&[SshFile::Target]).unwrap().status, 0);
    assert!(!files.exists(SshFile::Target).unwrap());
}
