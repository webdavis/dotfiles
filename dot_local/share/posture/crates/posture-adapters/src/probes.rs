use crate::{CommandIo, CommandRunner, SystemRunner, legacy_json::command_text};
use posture_domain::{
    Control, ControlReader, ControlReading, ControlValue, LuluProfile, classify_autologin,
    classify_filevault, classify_messages, classify_pgrep,
};
use std::ffi::OsStr;
use std::path::Path;
mod lulu;
use std::path::PathBuf;
use std::time::Duration;

// The Bash monitor bounds each status invocation at 20 seconds. This is not a tick budget.
pub(crate) const POLL_PROBE_BUDGET: Duration = Duration::from_secs(20);
pub struct ControlProbes<R = SystemRunner> {
    runner: R,
    uid: u32,
    rules: PathBuf,
    preferences: PathBuf,
}
impl ControlProbes<SystemRunner> {
    pub fn new(uid: u32, rules: PathBuf, preferences: PathBuf) -> Self {
        Self {
            runner: SystemRunner::per_command(POLL_PROBE_BUDGET),
            uid,
            rules,
            preferences,
        }
    }
}
impl<R: CommandRunner> ControlProbes<R> {
    pub fn read(&mut self, controls: &[Control]) -> (Vec<ControlReading>, LuluProfile) {
        // The shell preflights the base profile before any control reads, once per batch.
        let profile = if controls
            .iter()
            .any(|control| control.reader().requires_target())
        {
            self.profile()
        } else {
            LuluProfile::Base
        };
        let readings = controls
            .iter()
            .map(|control| self.control(control, profile))
            .collect();
        (readings, profile)
    }
    fn output(&mut self, program: &str, args: &[&OsStr], merged: bool) -> Option<(String, i32)> {
        let completed = self
            .runner
            .run_completed(
                Path::new(program),
                args,
                CommandIo::Inspection {
                    merge_stderr: merged,
                },
            )
            .ok()?;
        Some((
            command_text(String::from_utf8_lossy(&completed.bytes).into_owned()),
            completed.exit,
        ))
    }
    fn control(&mut self, control: &Control, profile: LuluProfile) -> ControlReading {
        use ControlReader::*;
        use ControlReading::Indeterminate;
        use ControlValue::{Disabled, Enabled};
        let reader = control.reader();
        if reader.requires_target() {
            return self.rule(control, profile);
        }
        let uid = self.uid.to_string();
        let (program, args): (&str, Vec<&OsStr>) = match reader {
            FileVault => ("/usr/bin/fdesetup", vec![OsStr::new("status")]),
            SystemIntegrity => ("/usr/bin/csrutil", vec![OsStr::new("status")]),
            AutoLogin => (
                "/usr/bin/defaults",
                [
                    "read",
                    "/Library/Preferences/com.apple.loginwindow",
                    "autoLoginUser",
                ]
                .map(OsStr::new)
                .to_vec(),
            ),
            GuestAccount => (
                "/usr/sbin/sysadminctl",
                ["-guestAccount", "status"].map(OsStr::new).to_vec(),
            ),
            OverSight => (
                "/usr/bin/pgrep",
                ["-x", "-U", &uid, "OverSight"].map(OsStr::new).to_vec(),
            ),
            LuluExtension => (
                "/usr/bin/pgrep",
                ["-x", "-U", "0", "com.objective-see.lulu.extension"]
                    .map(OsStr::new)
                    .to_vec(),
            ),
            LuluRule | LuluResolvedRule => unreachable!(),
        };
        let Some((output, exit)) = self.output(program, &args, true) else {
            return Indeterminate;
        };
        match reader {
            FileVault => classify_filevault(&output, exit),
            SystemIntegrity => classify_messages(
                &output,
                exit,
                &[
                    ("System Integrity Protection status: enabled.", Enabled),
                    ("System Integrity Protection status: disabled.", Disabled),
                ],
            ),
            AutoLogin => classify_autologin(&output, exit),
            GuestAccount => classify_messages(
                &output,
                exit,
                &[
                    ("Guest account enabled.", Enabled),
                    ("Guest account disabled.", Disabled),
                ],
            ),
            OverSight | LuluExtension => classify_pgrep(&output, exit),
            LuluRule | LuluResolvedRule => unreachable!(),
        }
    }
}
#[cfg(test)]
mod tests;
