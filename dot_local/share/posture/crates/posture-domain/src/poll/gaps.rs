use super::{ControlsRead, LuluProfile, PollPage, TrioReading};
use crate::{Control, ControlValue, Severity, controls::control_span};

pub(super) fn page(
    trio: TrioReading<'_>,
    clean: bool,
    controls: ControlsRead<'_>,
    readings: &[(&Control, Option<ControlValue>)],
    covered: &[&str],
    profile: LuluProfile,
) -> (Vec<String>, Option<PollPage>) {
    let mut members: Vec<String> = Vec::new();
    let mut details = Vec::new();
    if !clean {
        members.push("posture_query".into());
        // A failed query never contributes even plausible printed values.
        let values = if trio.exit == 0 { trio.values } else { [""; 3] };
        let [fw, gk, sl] = values.map(control_span);
        details.push(format!("the posture query returned an unreadable value (firewall={fw} gatekeeper={gk} screenlock={sl})"));
    }
    if let ControlsRead::Refused(refusal) = controls {
        members.push("controls_file".into());
        details.push(refusal.explanation.clone());
    }
    let indeterminate: Vec<_> = readings
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(control, _)| control.id.as_str())
        .collect();
    if !indeterminate.is_empty() {
        members.extend(indeterminate.iter().map(|id| (*id).into()));
        details.push(format!(
            "indeterminate posture control read(s): [{}]",
            indeterminate.join(" ")
        ));
    }
    if readings
        .iter()
        .any(|(control, _)| control.reader.requires_target())
    {
        match profile {
            LuluProfile::Active => details.push("a LuLu profile is ACTIVE (the base preferences carry a currentProfile key), so LuLu is consulting the profile's own files and the base rules archive these controls read is not the one deciding traffic".into()),
            LuluProfile::Unconfirmed => details.push("the LuLu base preferences could not be read to confirm no profile is active".into()),
            LuluProfile::Base => {}
        }
    }
    let alert = members.iter().any(|member| !covered.contains(&member.as_str())).then(|| PollPage {
        severity: Severity::Critical,
        title: "🔴 **CRITICAL**".into(),
        body: format!("**Security-posture monitoring gap**\n- {}: the security posture there is currently UNKNOWN.\n- A blind monitor cannot see a protection turn off. Did osqueryi, a posture probe, or the LaunchAgent break? **Check now.**\n- Diagnose: run the posture query and the control probes by hand, then re-check.", details.join("; ")),
        sound: "Sosumi",
    });
    (members, alert)
}
