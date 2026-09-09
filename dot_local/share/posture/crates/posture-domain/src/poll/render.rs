use super::{PollPage, Trio};
use crate::{Control, ControlValue, Severity};

pub(super) fn exposure_page(blocks: Vec<String>) -> Option<PollPage> {
    if blocks.is_empty() {
        return None;
    }
    let title = if blocks.len() == 1 {
        "🔴 **CRITICAL**".into()
    } else {
        format!("🔴 **CRITICAL** · {}", blocks.len())
    };
    Some(PollPage {
        severity: Severity::Critical,
        title,
        body: blocks.join("\n\n"),
        sound: "Sosumi",
    })
}

pub(super) fn trio_blocks(now: Trio, before: Option<Trio>) -> Vec<String> {
    let mut blocks = Vec::new();
    for (index, current) in now.0.into_iter().enumerate() {
        if current == 0 && before.is_none_or(|prior| prior.0[index] != 0) {
            blocks.push(builtin_block(index, before.map(|prior| prior.0[index])));
        }
    }
    blocks
}

fn builtin_block(index: usize, before: Option<u8>) -> String {
    let (name, value, remedy, first) = match index {
        0 => (
            "Firewall",
            "OFF",
            "System Settings → Network → Firewall",
            "The monitor has no prior baseline and the firewall is already off, a pre-existing exposure. Did you turn it off? If not, **investigate now**.",
        ),
        1 => (
            "Gatekeeper",
            "DISABLED",
            "System Settings → Privacy & Security (spctl cannot enable Gatekeeper from the CLI on macOS 15+)",
            "The monitor has no prior baseline and Gatekeeper is already disabled, a pre-existing exposure. Did you turn it off? If not, **investigate now**.",
        ),
        _ => (
            "Screen lock",
            "OFF",
            "System Settings → Lock Screen → Require password",
            "The monitor has no prior baseline and the screen-lock password requirement is already off, anyone at the machine has access. Did you turn it off? If not, **investigate now**.",
        ),
    };
    match before {
        None => format!(
            "**{name} is OFF (first observation)**\n- **Now:** **{value}**\n- {first}\n- Re-enable it: {remedy}"
        ),
        Some(previous) => {
            let previous = match (index, previous) {
                (0, 2) => "on (block all)",
                (0, _) => "on (allow signed)",
                (1, _) => "enabled",
                _ => "on",
            };
            format!(
                "**{name} turned OFF**\n- **Was:** {previous}\n- **Now:** **{value}**\n- Did you turn this off? If not, something else did, **investigate now**.\n- Re-enable it: {remedy}"
            )
        }
    }
}

pub(super) fn control_block(
    control: &Control,
    value: ControlValue,
    before: Option<ControlValue>,
) -> String {
    // Admission already sanitized and bounded these declaration fields. The
    // span delimiters are ours, and raw probe text never reaches this block.
    let description = &control.description;
    let expect = control.expect.as_str();
    let value = value.as_str();
    let mut block = match before {
        None => format!(
            "**`{description}`: {value} at first observation, declared {expect}**\n- **Now:** **{value}**\n- The monitor has no prior baseline for this control and it already deviates from its declared value, a pre-existing exposure. Did you change it? If not, **investigate now**."
        ),
        Some(previous) => format!(
            "**`{description}`: now {value}, declared {expect}**\n- **Was:** {}\n- **Now:** **{value}**\n- Did you change this? If not, something else did, **investigate now**.",
            previous.as_str()
        ),
    };
    if !control.remedy.is_empty() {
        block.push_str(&format!("\n- `{}`", control.remedy));
    }
    block
}
