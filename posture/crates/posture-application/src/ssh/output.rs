use super::SshVerification;
use std::io::Write;

pub struct SshOutput<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

impl SshOutput<'_> {
    pub fn info(&mut self, text: &str) -> bool {
        writeln!(self.stdout, "[ssh-hardening] {text}")
            .and_then(|()| self.stdout.flush())
            .is_ok()
    }
    pub fn warning(&mut self, text: &str) {
        let _ = writeln!(self.stderr, "[ssh-hardening] WARNING: {text}");
    }
    pub fn fail(&mut self, text: &str) -> u8 {
        let _ = writeln!(self.stderr, "[ssh-hardening] ERROR: {text}");
        1
    }
    pub fn verification(&mut self, result: &SshVerification) -> bool {
        match result {
            SshVerification::Verified => self.info(&format!("verify: PASS: all {} protected directives hold globally, no Match block in the include graph re-enables any of them, both sampled connections resolve hardened, and all {} sampled arrival addresses resolve the refusal verdict policy demands.", posture_domain::ssh_directive_count(), posture_domain::SSH_LOCAL_ADDRESS_SAMPLES.len())),
            SshVerification::Skipped => self.info("verify SKIPPED: sshd is not executable and the SSH_HARDENING_ALLOW_MISSING_SSHD test seam is set. The configuration was NOT checked."),
            SshVerification::Failed(failures) => {
                let _ = writeln!(self.stderr, "[ssh-hardening] verify FAILED, {} problem(s):", failures.len());
                for failure in failures { let _ = writeln!(self.stderr, "  - {failure}"); }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InspectionFailure;

    #[test]
    fn fatal_errors_keep_the_error_prefix() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut output = SshOutput {
            stdout: &mut out,
            stderr: &mut err,
        };
        assert_eq!(output.fail("could not stage"), 1);
        assert_eq!(err, b"[ssh-hardening] ERROR: could not stage\n");
    }

    #[test]
    fn timeouts_are_named_explicitly() {
        assert!(
            super::super::verify::command_failure(Err(InspectionFailure::TimedOut))
                .contains("TIMED OUT")
        );
    }

    #[test]
    fn failed_verification_retains_its_summary_and_bullet_diagnostics() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert!(
            !SshOutput {
                stdout: &mut out,
                stderr: &mut err
            }
            .verification(&SshVerification::Failed(vec!["fixture problem".into()]))
        );
        assert!(out.is_empty());
        assert_eq!(
            err,
            b"[ssh-hardening] verify FAILED, 1 problem(s):\n  - fixture problem\n"
        );
    }
}
