use super::*;

struct OwnedWizard(Child);
impl Drop for OwnedWizard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        // SAFETY: spawn created this child's new process group. Only that
        // owned group is signalled, then its direct child is reaped.
        unsafe {
            libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.0.wait();
    }
}

#[test]
fn setup_warns_before_secrets_about_managed_replacement_and_secret_diffs() {
    let expires = Instant::now() + Duration::from_millis(700);
    let sandbox = Sandbox::without_config("setup-managed-warning");
    let mut command = sandbox.bare();
    for (name, leaf) in [
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
        let path = sandbox.root.join(leaf);
        std::fs::create_dir_all(&path).unwrap();
        command.env(name, path);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["setup"])
        .process_group(0);
    let mut pty = Pty::open();
    let mut child = OwnedWizard(pty.spawn(&mut command));
    let remaining = || expires.saturating_duration_since(Instant::now());
    pty.read_until("or press enter to pair later: ", remaining())
        .expect("first secret prompt");
    // End the existing read with invalid UTF-8. This reaches no destination
    // and publishes nothing, even if the warning implementation is removed.
    pty.write_all(&[0xff, b'\n']);
    pty.read_to_eof(remaining())
        .expect("wizard exits after invalid input");
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < expires,
            "owned wizard exceeded its deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let warning = pty
        .transcript
        .find("a chezmoi-managed config is replaced on the next apply");
    assert!(
        warning.is_some(),
        "managed replacement warning was absent: {:?}",
        pty.transcript
    );
    assert!(warning.unwrap() < pty.transcript.find("Paste moshi's webhook secret").unwrap());
    assert!(
        pty.transcript.contains("diffs can expose the secrets"),
        "secret diff warning was absent"
    );
}
