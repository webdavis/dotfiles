//! The ledger, read as a checklist: which OPEN items owe an apply, and which
//! ones only the operator can do.
//!
//! A ledger is markdown written for people, so the parse stays deliberately
//! shallow: an open item is a `- [ ]` bullet, it owns the indented lines under
//! it, and it is classified by the phrases the config names. A phrase that
//! stops matching costs one missing row, never a failed run.

/// What one read of a ledger found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    /// Open items that mention an apply.
    pub owed_applies: Vec<String>,
    /// Open items that mention the operator.
    pub operator_items: Vec<String>,
}

/// Classifies every open item of a ledger against two sets of phrases, matched
/// case-insensitively against the item and its continuation lines.
pub fn parse(ledger: &str, apply_markers: &[String], operator_markers: &[String]) -> Ledger {
    let mut found = Ledger::default();
    for item in open_items(ledger) {
        let haystack = item.text.to_lowercase();
        if mentions(&haystack, apply_markers) {
            found.owed_applies.push(item.label.clone());
        }
        if mentions(&haystack, operator_markers) {
            found.operator_items.push(item.label);
        }
    }
    found
}

fn mentions(haystack: &str, markers: &[String]) -> bool {
    markers
        .iter()
        .any(|marker| haystack.contains(&marker.to_lowercase()))
}

/// One unchecked checklist item: its first line, and its whole body.
struct Item {
    label: String,
    text: String,
}

/// Every `- [ ]` bullet with the indented lines that belong to it. A checked
/// item, another bullet, a heading or a table row closes the one above it.
fn open_items(ledger: &str) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut open = false;
    for line in ledger.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- [ ]") {
            let label = rest.trim().to_string();
            items.push(Item {
                text: label.clone(),
                label,
            });
            open = true;
        } else if closes_an_item(trimmed) {
            open = false;
        } else if open && is_continuation(line) {
            if let Some(current) = items.last_mut() {
                current.text.push(' ');
                current.text.push_str(trimmed);
            }
        } else {
            open = false;
        }
    }
    items
}

fn closes_an_item(trimmed: &str) -> bool {
    trimmed.starts_with("- [x]") || trimmed.starts_with("- ") || trimmed.starts_with('#')
}

/// A continuation line is indented and non-blank; a blank line or unindented
/// prose closes the item instead of joining it.
fn is_continuation(line: &str) -> bool {
    (line.starts_with(' ') || line.starts_with('\t')) && !line.trim().is_empty()
}

#[cfg(test)]
mod tests;
