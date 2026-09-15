use super::*;
mod interrupt;
use posture_domain::SSH_LOCAL_ADDRESS_SAMPLES;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static INSTALLS: std::sync::Mutex<()> = std::sync::Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    config: Configuration,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "posture-ssh-flow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("dropins")).unwrap();
        fs::write(root.join("main"), "").unwrap();
        fs::write(
            root.join("dropins/000-ssh-hardening.conf"),
            "# prior target\n",
        )
        .unwrap();
        let quote = |path: PathBuf| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
        let target = quote(root.join("dropins/000-ssh-hardening.conf"));
        let fail = quote(root.join("refuse-verify"));
        // The stub resolves the refusal verdict the way the real sshd does: off the
        // laddr the spec carries, and only while the drop-in that holds the Match
        // block is installed. An unlisted laddr answers a value no policy wants, so
        // a sample the stub was never taught fails loudly instead of passing.
        let verdicts: String = SSH_LOCAL_ADDRESS_SAMPLES
            .iter()
            .map(|(address, verdict)| {
                format!("*laddr={address}*) printf 'refuseconnection {verdict}\\n';;\n")
            })
            .collect();
        let script = format!(
            "#!/bin/sh\nset -eu\n[ ! -e {fail} ] || exit 7\n[ \"$1\" != '-t' ] || exit 0\nprintf 'port 22\\n'\nif [ -e {target} ]; then\nprintf 'passwordauthentication no\\nkbdinteractiveauthentication no\\nusepam yes\\npubkeyauthentication yes\\npermitrootlogin no\\ngssapiauthentication no\\nhostbasedauthentication no\\n'\ncase \"$*\" in\n{verdicts}*laddr=*) printf 'refuseconnection unlisted\\n';;\nesac\nelse\nprintf 'passwordauthentication yes\\nkbdinteractiveauthentication no\\n'\ncase \"$*\" in *laddr=*) printf 'refuseconnection no\\n';; esac\nfi\n"
        );
        executable(&root.join("sshd"), &script);
        executable(
            &root.join("launchctl"),
            &format!(
                "#!/bin/sh\nset -eu\ncase \"$1\" in print) exit 0;; kickstart) printf restarted > {};; *) exit 90;; esac\n",
                quote(root.join("restarted"))
            ),
        );
        executable(
            &root.join("keyscan"),
            "#!/bin/sh\nprintf 'localhost ssh-ed25519 fixture-key\\n'\n",
        );
        let config = Configuration::read(|key| match key {
            "SSHD_CONFIG_D" => Some(root.join("dropins").into_os_string()),
            "SSHD_MAIN_CONFIG" => Some(root.join("main").into_os_string()),
            "SSHD_BIN" => Some(root.join("sshd").into_os_string()),
            "LAUNCHCTL_BIN" => Some(root.join("launchctl").into_os_string()),
            "KEYSCAN_BIN" => Some(root.join("keyscan").into_os_string()),
            "SSH_HARDENING_SUDO" => Some("".into()),
            "SSH_HARDENING_READY_ATTEMPTS" => Some("1".into()),
            "SSH_HARDENING_READY_INTERVAL" => Some("0".into()),
            _ => None,
        });
        Self { root, config }
    }
    fn run(&self, verb: Verb) -> (u8, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let status = perform(
            verb,
            &self.config,
            &|| Some("fixture-user".into()),
            &mut SshOutput {
                stdout: &mut out,
                stderr: &mut err,
            },
        );
        (
            status,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // These tests install real files, so without an owner each run leaves another tree in the
        // system temporary directory forever. Best effort on purpose: a panicking drop during a
        // failing test would abort the run and hide the assertion that actually failed.
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn executable(path: &std::path::Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn native_private_verify_reload_install_and_rollback_follow_their_owned_contracts() {
    let _serial = INSTALLS.lock().unwrap();
    let f = Fixture::new();
    let (status, out, err) = f.run(Verb::Verify);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("verify: PASS: all 7"));
    // The PASS line names the arrival-address samples too, from the same list
    // the verify resolved, so a sample added to policy cannot leave the line
    // claiming a number nobody checked.
    assert!(
        out.contains(&format!(
            "all {} sampled arrival addresses",
            posture_domain::SSH_LOCAL_ADDRESS_SAMPLES.len()
        )),
        "{out}"
    );
    assert!(!f.root.join("restarted").exists());
    let (status, out, err) = f.run(Verb::Reload);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("reload complete"));
    assert_eq!(fs::read(f.root.join("restarted")).unwrap(), b"restarted");
    let (status, out, err) = f.run(Verb::Install);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("install complete"));
    assert_eq!(
        fs::read(f.root.join("dropins/000-ssh-hardening.conf")).unwrap(),
        posture_domain::ssh_config().as_bytes()
    );
    let (status, out, err) = f.run(Verb::Rollback);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("at the next sshd start"));
    assert!(!f.root.join("dropins/000-ssh-hardening.conf").exists());
}
#[test]
fn native_failed_install_restores_the_saved_symlink_and_legacy_bytes() {
    let _serial = INSTALLS.lock().unwrap();
    let f = Fixture::new();
    let target = f.root.join("dropins/000-ssh-hardening.conf");
    fs::rename(&target, f.root.join("prior-target")).unwrap();
    symlink(f.root.join("prior-target"), &target).unwrap();
    fs::write(
        f.root.join("dropins/50-no-password-auth.conf"),
        "# prior legacy\n",
    )
    .unwrap();
    fs::write(f.root.join("refuse-verify"), "").unwrap();
    let (status, out, err) = f.run(Verb::Install);
    assert_eq!(status, 1);
    assert!(err.contains("verify FAILED"), "{err}");
    assert!(!out.contains("install complete"));
    assert_eq!(fs::read_link(&target).unwrap(), f.root.join("prior-target"));
    assert_eq!(fs::read(target).unwrap(), b"# prior target\n");
    assert_eq!(
        fs::read(f.root.join("dropins/50-no-password-auth.conf")).unwrap(),
        b"# prior legacy\n"
    );
    assert!(!f.root.join("restarted").exists());
}

#[test]
fn rollback_removes_the_target_before_asking_for_recovery_identity() {
    let f = Fixture::new();
    let target = f.root.join("dropins/000-ssh-hardening.conf");
    let lookup = || {
        assert!(
            !target.exists(),
            "recovery identity must not delay the emergency removal"
        );
        Some("fixture-user".into())
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        perform(
            Verb::Rollback,
            &f.config,
            &lookup,
            &mut SshOutput {
                stdout: &mut out,
                stderr: &mut err
            }
        ),
        0,
        "{}",
        String::from_utf8_lossy(&err)
    );
}

#[test]
fn one_verification_budget_covers_all_resolutions_and_prevents_a_late_third_spawn() {
    use std::time::Duration;
    let mut f = Fixture::new();
    // The budget has to outlast one resolution and die inside the second, and it also has to
    // leave a spawn on a loaded machine far more room than it needs, so both are stated in
    // hundreds of milliseconds rather than tens.
    f.config.deadline = Duration::from_millis(250);
    f.config.grace = Duration::from_millis(5);
    let script = fs::read_to_string(f.root.join("sshd")).unwrap();
    let calls = f.root.join("calls");
    executable(
        &f.root.join("sshd"),
        &script.replacen(
            "set -eu\n",
            &format!(
                "set -eu\nprintf 'call\\n' >> '{}'\n/bin/sleep 0.15\n",
                calls.to_str().unwrap().replace('\'', "'\\''")
            ),
            1,
        ),
    );
    let (status, out, err) = f.run(Verb::Verify);
    assert_eq!(status, 1);
    assert!(err.contains("124"), "{err}");
    assert!(!out.contains("PASS"));
    // A per-command budget would let all three resolutions finish inside it and a budget that
    // covered none of them would start no resolution at all; only the aggregate one lands between
    // the two, so the count is what discriminates and it is bounded on both sides.
    let started = fs::read_to_string(calls).unwrap().lines().count();
    assert!(
        (1..=2).contains(&started),
        "one aggregate budget must cover the first resolution and refuse a third: {started}"
    );
}
