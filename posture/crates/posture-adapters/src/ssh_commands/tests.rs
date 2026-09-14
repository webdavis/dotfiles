use super::*;
use crate::CommandOutput;
use posture_application::{SshBannerProbe, SshFile, SshInstallFiles, SshLaunchctl, Sshd};
use std::{cell::RefCell, ffi::OsString, path::PathBuf, rc::Rc};

#[derive(Debug, PartialEq, Eq)]
struct Call {
    executable: PathBuf,
    args: Vec<OsString>,
    input: Option<Vec<u8>>,
    interactive: bool,
}
#[derive(Clone, Default)]
struct Runner(Rc<RefCell<Vec<Call>>>);
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        executable: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        self.0.borrow_mut().push(Call {
            executable: executable.into(),
            args: args.iter().map(|a| a.to_os_string()).collect(),
            input: match io {
                CommandIo::Input(bytes) => Some(bytes.to_vec()),
                _ => None,
            },
            interactive: io == CommandIo::InheritAll,
        });
        Ok(CommandOutput {
            exit: 113,
            bytes: b"fixture output".to_vec(),
        })
    }
}

#[test]
fn verification_never_escalates_and_syntax_uses_the_owned_privilege_boundary() {
    let runner = Runner::default();
    let mut sshd = SshdCommand::new(
        runner.clone(),
        "/private/sshd".into(),
        "/private/main config".into(),
        Some("/private/sudo".into()),
    );
    for result in [
        sshd.global(),
        sshd.connection("user=alice,host=localhost,addr=127.0.0.1"),
        sshd.syntax(),
    ] {
        assert_eq!(
            result,
            Ok(SshCompleted {
                status: 113,
                output: b"fixture output".to_vec()
            })
        );
    }
    assert_eq!(
        *runner.0.borrow(),
        [
            call("/private/sshd", &["-G", "-f", "/private/main config"]),
            call(
                "/private/sshd",
                &[
                    "-G",
                    "-T",
                    "-C",
                    "user=alice,host=localhost,addr=127.0.0.1",
                    "-f",
                    "/private/main config"
                ]
            ),
            call(
                "/private/sudo",
                &["-n", "/private/sshd", "-t", "-f", "/private/main config"]
            ),
        ]
    );
}

#[test]
fn launchd_probe_is_unprivileged_so_wrapper_failure_cannot_masquerade_as_absence() {
    let runner = Runner::default();
    let mut launchd = SshLaunchd::new(
        runner.clone(),
        "/private/launchctl".into(),
        Some("/private/sudo".into()),
    );
    assert_eq!(launchd.probe().unwrap().status, 113);
    assert_eq!(launchd.restart().unwrap().status, 113);
    assert_eq!(
        *runner.0.borrow(),
        [
            call("/private/launchctl", &["print", "system/com.openssh.sshd"]),
            call(
                "/private/sudo",
                &[
                    "-n",
                    "/private/launchctl",
                    "kickstart",
                    "-k",
                    "system/com.openssh.sshd"
                ]
            )
        ]
    );
}

#[test]
fn banner_probe_passes_the_resolved_port_and_timeout_as_separate_arguments() {
    let runner = Runner::default();
    let mut keyscan = SshKeyscan::new(runner.clone(), "/private/keyscan".into());
    assert_eq!(keyscan.probe(2222, 5).unwrap().output, b"fixture output");
    assert_eq!(
        *runner.0.borrow(),
        [call(
            "/private/keyscan",
            &["-T", "5", "-p", "2222", "127.0.0.1"]
        )]
    );
}

#[test]
fn install_operations_preserve_types_use_absolute_tools_and_keep_staged_content_on_stdin() {
    let runner = Runner::default();
    let mut files = SshFileInstaller::new(
        runner.clone(),
        "/private/config".into(),
        Some("/private/sudo".into()),
    );
    for result in [
        files.prime(),
        files.stage(b"policy\n"),
        files.chmod(),
        files.save(),
        files.rename(SshFile::Staging, SshFile::Target),
        files.remove(&[SshFile::SavedTarget, SshFile::SavedLegacy]),
    ] {
        assert_eq!(result.unwrap().status, 113);
    }
    let mut prime = call("/private/sudo", &["-v"]);
    prime.interactive = true;
    let mut stage = call(
        "/private/sudo",
        &[
            "-n",
            "/usr/bin/tee",
            "--",
            "/private/config/.000-ssh-hardening.conf.staging",
        ],
    );
    stage.input = Some(b"policy\n".to_vec());
    assert_eq!(
        *runner.0.borrow(),
        [
            prime,
            stage,
            call(
                "/private/sudo",
                &[
                    "-n",
                    "/bin/chmod",
                    "--",
                    "0644",
                    "/private/config/.000-ssh-hardening.conf.staging"
                ]
            ),
            call(
                "/private/sudo",
                &[
                    "-n",
                    "/bin/cp",
                    "-Rp",
                    "--",
                    "/private/config/000-ssh-hardening.conf",
                    "/private/config/.000-ssh-hardening.conf.saved"
                ]
            ),
            call(
                "/private/sudo",
                &[
                    "-n",
                    "/bin/mv",
                    "-f",
                    "--",
                    "/private/config/.000-ssh-hardening.conf.staging",
                    "/private/config/000-ssh-hardening.conf"
                ]
            ),
            call(
                "/private/sudo",
                &[
                    "-n",
                    "/bin/rm",
                    "-f",
                    "--",
                    "/private/config/.000-ssh-hardening.conf.saved",
                    "/private/config/.50-no-password-auth.conf.saved"
                ]
            ),
        ]
    );
}

fn call(executable: &str, args: &[&str]) -> Call {
    Call {
        executable: executable.into(),
        args: args.iter().map(OsString::from).collect(),
        input: None,
        interactive: false,
    }
}

mod native;
