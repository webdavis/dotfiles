use super::*;
use crate::property_list::PropertyList;
use posture_domain::classify_lulu_profile;

impl<R: CommandRunner, P: ProcessLookup> ControlProbes<R, P> {
    pub(super) fn profile(&self) -> LuluProfile {
        classify_lulu_profile(
            PropertyList::read(&self.preferences).map(|list| list.declares("currentProfile")),
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
        let Some(rules) = PropertyList::read(&self.rules) else {
            return Indeterminate;
        };
        // The private keyed archive proves only path existence, never allow/block action.
        // Match a whole string value so a longer path cannot satisfy this control.
        Known(if rules.contains_string(&target) {
            ControlValue::Present
        } else {
            ControlValue::Absent
        })
    }
}
