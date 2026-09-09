use pns_domain::{Delivery, verdict};
use std::collections::BTreeSet;

pub(crate) fn revise_leg(
    line: &str,
    destination: &str,
    delivery: &Delivery,
) -> Result<String, &'static str> {
    if !name_is_safe(destination) {
        return Err("unusable decision destination name");
    }
    let body = line.strip_suffix('\n').ok_or(MALFORMED)?;
    let (prefix, suffix) = body.split_once(" legs=").ok_or(MALFORMED)?;
    if body.contains(['\n', '\r']) {
        return Err(MALFORMED);
    }
    let mut seen = BTreeSet::new();
    let mut found = false;
    let mut legs = Vec::new();
    if suffix != "none" {
        for entry in suffix.split(',') {
            let (name, previous) = entry.split_once(':').ok_or(MALFORMED)?;
            if !name_is_safe(name)
                || !matches!(previous, "delivered" | "failed" | "unlaunched" | "silent")
                || !seen.insert(name)
            {
                return Err(MALFORMED);
            }
            if name == destination {
                legs.push(format!("{name}:{}", verdict(delivery)));
                found = true;
            } else {
                legs.push(entry.to_string());
            }
        }
    }
    if !found {
        legs.push(format!("{destination}:{}", verdict(delivery)));
    }
    Ok(format!("{prefix} legs={}\n", legs.join(",")))
}

// Plugin identifiers may contain dots. Unlike the truncated identity fields,
// these names must remain exact; neither route nor pane validation has this grammar.
fn name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
const MALFORMED: &str = "malformed decision outcome field";
