//! The daily digest body: everything that did not page, grouped and capped.
//!
//! The page carries the one thing to act on now. This carries the rest, once a
//! day, silently, so a finding that was merely interesting is still seen
//! without teaching the operator to dismiss the ones that are not.
//!
//! GROUPED BY DETECTOR, because a day's spool is repetitive by nature: twenty
//! lines from one detector are one fact seen twenty times, and a flat list
//! renders that as twenty facts. The group's header carries the TRUE count even
//! when the bullets under it are capped, so the roll-up never hides how much
//! there was.
//!
//! DISPLAY ONLY. Nothing here reads the spool, claims a batch, rotates a file
//! or asks the clock; every one of those is the adapter's. What arrives is a
//! list of entries and what leaves is a string.

use crate::sanitize;

/// How many bullets one detector's group renders before it rolls the rest up.
///
/// The group header still states the true count, so this bounds the LINES a
/// noisy detector spends, never what the operator is told happened.
pub const BULLETS_PER_GROUP: usize = 10;

/// How many detector groups render before the rest collapse to one marker.
///
/// THE BODY CAP IS NOT ENOUGH ON ITS OWN. Without this, a busy day fills the
/// character budget with its first few groups and every trailing group is lost
/// to a mid-line cut that says nothing about what it swallowed. Capping the
/// count instead spends one line saying how many detectors went unshown.
pub const GROUP_LIMIT: usize = 12;

/// The whole body's hard cap, below the delivery limit it protects.
///
/// Lower than the page's, deliberately: the digest's title is longer and it has
/// no urgency to spend the margin on.
pub const BODY_LIMIT: usize = 1800;

/// What marks a body cut short.
const BODY_TRUNCATION: &str = "\n… (truncated)";

/// What a field the record did not carry renders as.
const UNKNOWN: &str = "?";

/// One spooled finding, as the digest reads it.
///
/// THREE FIELDS, not the six the record carries. The timestamp, the category
/// and the action are recorded for forensics and never rendered, and a type
/// that carried them would invite a later change to start rendering one without
/// noticing that the spool's privacy posture is what decided against it.
///
/// `Option` rather than an empty string, because a record whose detector is
/// absent and one whose detector is the empty string are different records; the
/// first is malformed and the second is a detector named nothing. Both render
/// the same, and both should, but the codec must not have to lie to say so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DigestEntry<'a> {
    pub detector: Option<&'a str>,
    pub identity: Option<&'a str>,
    pub summary: Option<&'a str>,
}

/// Render the spooled entries into one digest body.
///
/// AN EMPTY BODY IS A REAL ANSWER, returned when there was nothing to render.
/// The caller reads it as "say nothing today" rather than sending a message
/// whose count promises findings its body does not show.
pub fn render_digest(entries: &[DigestEntry<'_>]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let groups = grouped(entries);
    let mut blocks: Vec<String> = groups
        .iter()
        .take(GROUP_LIMIT)
        .map(|(detector, members)| block(*detector, members))
        .collect();
    if let Some(dropped) = groups.len().checked_sub(GROUP_LIMIT).filter(|n| *n > 0) {
        blocks.push(format!(
            "… and {dropped} more detector group(s) - see results.log"
        ));
    }
    capped(blocks.join("\n"))
}

/// The entries by detector, in detector order.
///
/// SORTED BY NAME rather than left in spool order, so the same day's findings
/// render the same way whichever order the alerter happened to append them in.
/// Within a group the spool's own order is kept, which is chronological, so the
/// bullets that survive the cap are the day's earliest rather than an arbitrary
/// ten.
///
/// A RECORD WITH NO DETECTOR GROUPS APART from one whose detector is literally
/// `?`, and sorts ahead of every named group. The two render identical headers,
/// which looks like a mistake and is not: they are different records, one
/// malformed and one naming a detector nobody has, and merging them would let a
/// crafted `"detector":"?"` line hide inside the malformed group's count. The
/// renderer this replaces behaves the same way, for the same reason `Option`
/// orders before `Some` here.
fn grouped<'a>(entries: &[DigestEntry<'a>]) -> Vec<(Option<&'a str>, Vec<DigestEntry<'a>>)> {
    let mut groups: Vec<(Option<&'a str>, Vec<DigestEntry<'a>>)> = Vec::new();
    for entry in entries {
        match groups.iter_mut().find(|(name, _)| *name == entry.detector) {
            Some((_, members)) => members.push(*entry),
            None => groups.push((entry.detector, vec![*entry])),
        }
    }
    groups.sort_by(|left, right| left.0.cmp(&right.0));
    groups
}

/// One detector's block: its header, its bullets, its roll-up, its separator.
///
/// THE TRAILING BLANK LINE IS PART OF THE BLOCK rather than a separator the
/// join adds, so the last group ends the body the same way the others end
/// theirs and the overflow marker below always sits on its own.
fn block(detector: Option<&str>, members: &[DigestEntry<'_>]) -> String {
    let mut lines = vec![format!(
        "**{}** ({})",
        detector.unwrap_or(UNKNOWN),
        members.len()
    )];
    for member in members.iter().take(BULLETS_PER_GROUP) {
        lines.push(format!(
            "- {} - {}",
            sanitize::code(member.identity.unwrap_or(UNKNOWN)),
            sanitize::code(member.summary.unwrap_or(UNKNOWN))
        ));
    }
    if let Some(dropped) = members
        .len()
        .checked_sub(BULLETS_PER_GROUP)
        .filter(|n| *n > 0)
    {
        lines.push(format!("… +{dropped} more"));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// The body, cut to `BODY_LIMIT` characters if it overruns.
///
/// CHARACTERS, not bytes, because a cut through a multi-byte character renders
/// a replacement glyph where the operator expects a path.
fn capped(body: String) -> String {
    if body.chars().count() <= BODY_LIMIT {
        return body;
    }
    let mut cut: String = body.chars().take(BODY_LIMIT).collect();
    cut.push_str(BODY_TRUNCATION);
    cut
}

#[cfg(test)]
mod tests;
