use super::*;
use posture_domain::classify_lulu_profile;

impl<R: CommandRunner> ControlProbes<R> {
    pub(super) fn profile(&mut self) -> LuluProfile {
        let preferences = self.preferences.clone();
        let Some((xml, exit)) = self.plist(&preferences) else {
            return LuluProfile::Unconfirmed;
        };
        classify_lulu_profile(
            exit == 0,
            !xml.is_empty(),
            xml.contains("<key>currentProfile</key>"),
        )
    }
    fn plist(&mut self, path: &Path) -> Option<(String, i32)> {
        self.output(
            "/usr/bin/plutil",
            &[
                OsStr::new("-convert"),
                OsStr::new("xml1"),
                OsStr::new("-o"),
                OsStr::new("-"),
                path.as_os_str(),
            ],
            false,
        )
    }
    pub(super) fn rule(&mut self, control: &Control, profile: LuluProfile) -> ControlReading {
        use ControlReading::{Indeterminate, Known};
        // An active or unreadable profile makes the base archive non-authoritative.
        // Neither launcher resolution nor the archive read may run in that case.
        if profile != LuluProfile::Base {
            return Indeterminate;
        }
        let mut target = control.target().to_owned();
        if control.reader() == ControlReader::LuluResolvedRule {
            let Some((resolved, exit)) = self.output(
                "/usr/bin/readlink",
                &[OsStr::new("-f"), OsStr::new(&target)],
                false,
            ) else {
                return Indeterminate;
            };
            if exit != 0 || resolved.is_empty() {
                return Indeterminate;
            }
            target = resolved;
        }
        let rules = self.rules.clone();
        let Some((xml, exit)) = self.plist(&rules) else {
            return Indeterminate;
        };
        if exit != 0 || xml.is_empty() {
            return Indeterminate;
        }
        // The private keyed archive proves only path existence, never allow/block action.
        // Match the exact escaped element so a longer path cannot satisfy this control.
        let escaped = target
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        Known(if xml.contains(&format!("<string>{escaped}</string>")) {
            ControlValue::Present
        } else {
            ControlValue::Absent
        })
    }
}
