use super::{SshCommandResult, SshTree, Sshd};
use crate::InspectionFailure;
use posture_domain::{SSH_LOCAL_ADDRESS_SAMPLES, SshJudgment, judge_ssh_output, judge_ssh_refusal};
use std::path::Path;
mod scan_failure;

pub(super) fn tree_failure(failure: super::SshScanFailure) -> String {
    scan_failure::describe(failure)
}

#[derive(Debug, PartialEq, Eq)]
pub enum SshVerification {
    Verified,
    Skipped,
    Failed(Vec<String>),
}

pub struct SshVerifyContext<'a> {
    pub user: &'a dyn Fn() -> Option<String>,
    pub executable: &'a Path,
    pub allow_missing: bool,
}

pub fn verify_ssh(
    sshd: &mut impl Sshd,
    tree: &impl SshTree,
    context: &SshVerifyContext<'_>,
) -> SshVerification {
    if !sshd.available() {
        return if context.allow_missing {
            SshVerification::Skipped
        } else {
            SshVerification::Failed(vec![format!(
                "FAILING CLOSED: {} is not executable, so the effective configuration cannot be checked. Refusing to guess.",
                context.executable.display()
            )])
        };
    }
    let mut failures = Vec::new();
    check_output(
        "global check",
        sshd.global(),
        judge_ssh_output,
        &mut failures,
    );
    failures.extend(tree.scan().into_iter().map(scan_failure::describe));
    if let Some(user) = (context.user)().filter(|name| !name.is_empty()) {
        // No laddr: these resolve in the drop-in's permissive branch, which is
        // where the seven protected directives are judged.
        for name in ["root", user.as_str()] {
            let spec = format!("user={name},host=localhost,addr=127.0.0.1");
            check_output(
                &format!("connection check ({spec})"),
                sshd.connection(&spec),
                judge_ssh_output,
                &mut failures,
            );
        }
        // With laddr: the arrival address decides the refusal verdict, and the
        // client address is a documentation address precisely because it is not
        // the criterion.
        for (address, verdict) in SSH_LOCAL_ADDRESS_SAMPLES {
            let spec = format!("user={user},host=localhost,addr=203.0.113.1,laddr={address}");
            check_output(
                &format!("local address check ({spec})"),
                sshd.connection(&spec),
                |output| judge_ssh_refusal(output, verdict),
                &mut failures,
            );
        }
    } else {
        failures.push("connection check: could not determine the invoking user; failing closed rather than probing a spec built from an empty name".into());
    }
    let mut unique: Vec<String> = Vec::new();
    for failure in failures {
        if !unique.iter().any(|old| old.eq_ignore_ascii_case(&failure)) {
            unique.push(failure);
        }
    }
    if unique.is_empty() {
        SshVerification::Verified
    } else {
        SshVerification::Failed(unique)
    }
}

fn check_output(
    label: &str,
    result: SshCommandResult,
    judge: impl FnOnce(&[u8]) -> Vec<SshJudgment>,
    failures: &mut Vec<String>,
) {
    match result {
        Ok(completed) if completed.status == 0 => failures.extend(
            judge(&completed.output)
                .iter()
                .map(|judgment| describe_judgment(label, judgment)),
        ),
        other => failures.push(format!(
            "{label}: {}; failing closed",
            command_failure(other)
        )),
    }
}

pub(super) fn command_failure(result: SshCommandResult) -> String {
    match result {
        Ok(completed) => format!(
            "command exited {} (output: {})",
            completed.status,
            String::from_utf8_lossy(&completed.output).trim_end_matches('\n')
        ),
        Err(InspectionFailure::TimedOut) => {
            "command TIMED OUT: reached its deadline and was stopped (exit 124)".into()
        }
        Err(InspectionFailure::Unavailable) => "command could not run (exit 127)".into(),
        Err(InspectionFailure::Failed) => "command could not be read or completed (exit 1)".into(),
    }
}

fn describe_judgment(label: &str, judgment: &SshJudgment) -> String {
    let key = judgment.keyword;
    match &judgment.actual {
        None => format!("{label}: '{key}' is absent from the effective configuration"),
        Some(actual) => format!(
            "{label}: '{key}' is '{}', want '{}'",
            String::from_utf8_lossy(actual),
            judgment.required
        ),
    }
}

#[cfg(test)]
mod tests;
