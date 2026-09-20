use super::*;
use std::{
    io::Read,
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn a_real_install_interrupt_stops_verification_descendants_restores_files_and_reraises() {
    const MARKER: &str = "POSTURE_PRIVATE_INSTALL_INTERRUPT";
    if let Some(root) = std::env::var_os(MARKER) {
        // A disposition survives fork and exec, so a parent that ignores TERM (nohup, a launchd
        // job) would leave this fixture unable to receive the interrupt it exists to measure.
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            assert_ne!(
                unsafe { libc::signal(signal, libc::SIG_DFL) },
                libc::SIG_ERR
            );
        }
        let root = PathBuf::from(root);
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("posture-ssh-flow-")
        );
        let mut config = Configuration::read(|key| match key {
            "SSHD_CONFIG_D" => Some(root.join("dropins").into_os_string()),
            "SSHD_MAIN_CONFIG" => Some(root.join("main").into_os_string()),
            "SSHD_BIN" => Some(root.join("sshd").into_os_string()),
            "SSH_HARDENING_SUDO" => Some("".into()),
            _ => None,
        });
        config.grace = Duration::from_millis(10);
        config.deadline = Duration::from_millis(250);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let status = perform(
            Verb::Install,
            &config,
            &|| Some("fixture-user".into()),
            &mut SshOutput {
                stdout: &mut out,
                stderr: &mut err,
            },
        );
        panic!(
            "install should re-raise its signal, got {status}: {}",
            String::from_utf8_lossy(&err)
        );
    }
    let f = Fixture::new();
    fs::write(
        f.root.join("dropins/50-no-password-auth.conf"),
        "# prior legacy\n",
    )
    .unwrap();
    let pids = f.root.join("owned-pids");
    executable(
        &f.root.join("sshd"),
        &format!(
            "#!/bin/sh\ntrap '' TERM\n/bin/sh -c 'trap \"\" TERM; while :; do :; done' &\nprintf '%s\\n%s\\n' \"$$\" \"$!\" > '{}'\n/bin/kill -TERM \"$PPID\"\nwait\n",
            pids.to_str().unwrap().replace('\'', "'\\''")
        ),
    );
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","ssh::native::tests::interrupt::a_real_install_interrupt_stops_verification_descendants_restores_files_and_reraises","--nocapture"])
        .env_clear().env(MARKER, f.root.path()).env("TMPDIR",std::env::temp_dir()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    // The fixture's own verification budget ends it in tens of milliseconds. This is a watchdog
    // rather than a bound on that: a regression that never terminates the group would otherwise
    // hang the suite, and no load on this machine puts a signalled exit ten seconds out.
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    let owned: Vec<u32> = fs::read_to_string(&pids)
        .unwrap_or_default()
        .lines()
        .map(|v| v.parse().unwrap())
        .collect();
    // Reaping the killed group is the kernel's own work, so it is polled for rather than timed:
    // the loop leaves as soon as they are gone and the deadline only bounds a failure.
    let absent_by = Instant::now() + Duration::from_secs(5);
    let alive = loop {
        let alive = owned.iter().any(|pid| {
            Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        });
        if !alive || Instant::now() >= absent_by {
            break alive;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    if alive && let Some(leader) = owned.first() {
        // A failed mutant may leave this test's TERM-ignoring verifier group behind.
        let _ = Command::new("/bin/kill")
            .args(["-9", &format!("-{leader}")])
            .stderr(Stdio::null())
            .status();
    }
    assert_eq!(owned.len(), 2, "{err}");
    assert!(!alive, "owned verification processes survived: {owned:?}");
    assert_eq!(status.and_then(|s| s.signal()), Some(15), "{err}");
    assert_eq!(
        fs::read(f.root.join("dropins/000-ssh-hardening.conf")).unwrap(),
        b"# prior target\n"
    );
    assert_eq!(
        fs::read(f.root.join("dropins/50-no-password-auth.conf")).unwrap(),
        b"# prior legacy\n"
    );
    assert!(
        !f.root
            .join("dropins/.000-ssh-hardening.conf.staging")
            .exists()
    );
}
