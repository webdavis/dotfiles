//! The recap document on the wire: JSON out, TOON out, and a field mask read
//! back in from either.
//!
//! TWO FORMATS, ONE TREE. The document's shape is decided once in the domain,
//! so these only spell it; a field that appears in one form and not the other
//! would be a contract that depends on which flag the consumer typed.
//!
//! THE MASK'S FORMAT IS DETECTED FROM ITS CONTENT rather than from a file
//! extension, because `--schema -` is a pipe and has no name to read one off.

use pns_domain::recap::document::{Mask, Node};

/// Which spelling of the document a caller is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Json,
    Toon,
}

impl Wire {
    pub fn encode(self, document: &Node) -> String {
        match self {
            Wire::Json => json(document),
            Wire::Toon => toon(document),
        }
    }
}

/// The document as JSON.
///
/// WRITTEN DIRECTLY RATHER THAN THROUGH `serde_json::Value`, because that
/// map sorts its keys: the document's key ORDER is part of its shape, a
/// golden fixture pins bytes, and the one switch that would preserve it
/// (`preserve_order`) is a workspace-wide feature this repository keeps off on
/// purpose. Every string still goes through serde's own escaping, so nothing
/// here hand-rolls the part that is easy to get wrong.
fn json(node: &Node) -> String {
    match node {
        Node::Text(text) => serde_json::Value::String(text.clone()).to_string(),
        Node::Number(value) => value.to_string(),
        Node::Flag(value) => value.to_string(),
        Node::Absent => "null".to_string(),
        Node::List(items) => format!("[{}]", items.iter().map(json).collect::<Vec<_>>().join(",")),
        Node::Map(pairs) => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    serde_json::Value::String(key.clone()),
                    json(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// The document as TOON.
///
/// LIST FORM FOR AN ARRAY OF OBJECTS, never the tabular header. The one such
/// array this document carries is `agents`, whose elements hold a nested list
/// of their own, which the tabular form cannot express; writing the uniform
/// case one way and the nested case another would mean two encoders to keep
/// honest for no reader's benefit.
fn toon(node: &Node) -> String {
    let mut lines = Vec::new();
    write_toon(node, 0, &mut lines);
    lines.join("\n")
}

fn write_toon(node: &Node, depth: usize, lines: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    match node {
        Node::Map(pairs) => {
            for (key, value) in pairs {
                match value {
                    Node::Map(nested) if nested.is_empty() => lines.push(format!("{pad}{key}:")),
                    Node::Map(_) => {
                        lines.push(format!("{pad}{key}:"));
                        write_toon(value, depth + 1, lines);
                    }
                    Node::List(items) => write_list(key, items, depth, lines),
                    scalar => lines.push(format!("{pad}{key}: {}", scalar_toon(scalar))),
                }
            }
        }
        scalar => lines.push(format!("{pad}{}", scalar_toon(scalar))),
    }
}

fn write_list(key: &str, items: &[Node], depth: usize, lines: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    let count = items.len();
    if items.iter().all(|item| !matches!(item, Node::Map(_))) {
        let row = items.iter().map(scalar_toon).collect::<Vec<_>>().join(",");
        lines.push(format!("{pad}{key}[{count}]: {row}"));
        return;
    }
    lines.push(format!("{pad}{key}[{count}]:"));
    for item in items {
        let mut nested = Vec::new();
        write_toon(item, 0, &mut nested);
        let inner = "  ".repeat(depth + 1);
        for (which, line) in nested.into_iter().enumerate() {
            match which {
                0 => lines.push(format!("{inner}- {line}")),
                _ => lines.push(format!("{inner}  {line}")),
            }
        }
    }
}

/// One scalar, quoted whenever leaving it bare would read as something else.
fn scalar_toon(node: &Node) -> String {
    match node {
        Node::Number(value) => value.to_string(),
        Node::Flag(value) => value.to_string(),
        Node::Absent => "null".to_string(),
        Node::Text(text) => quoted(text),
        // A nested structure inside a primitive position cannot happen in this
        // document; spelling it as its own line keeps the function total.
        other => {
            let mut lines = Vec::new();
            write_toon(other, 0, &mut lines);
            lines.join(" ")
        }
    }
}

/// Whether a string has to be quoted, and the quoting itself.
///
/// THE RULE IS THE SPEC'S: an empty string, leading or trailing whitespace,
/// the three literals, anything that reads as a number, and any text holding
/// the delimiter, a colon, a quote or a backslash.
fn quoted(text: &str) -> String {
    let bare = !text.is_empty()
        && text.trim() == text
        && !matches!(text, "true" | "false" | "null")
        && text.parse::<f64>().is_err()
        && !text.contains([',', ':', '"', '\\', '\n', '[', ']', '{', '}']);
    match bare {
        true => text.to_string(),
        false => format!(
            "\"{}\"",
            text.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        ),
    }
}

/// A field mask read out of a document in either format, and which format it
/// was written in.
///
/// AN UNREADABLE MASK IS A REFUSAL rather than an empty one, which the caller
/// turns into exit 2: a mask nobody could parse would otherwise narrow the
/// document to nothing and read as a quiet window.
pub fn read_mask(text: &str) -> Option<(Mask, Wire)> {
    match text.trim_start().starts_with('{') {
        true => json_mask(text).map(|mask| (mask, Wire::Json)),
        false => Some((toon_mask(text), Wire::Toon)),
    }
}

fn json_mask(text: &str) -> Option<Mask> {
    fn walk(value: &serde_json::Value) -> Mask {
        Mask {
            fields: value
                .as_object()
                .map(|object| {
                    object
                        .iter()
                        .map(|(key, nested)| (key.clone(), walk(nested)))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
    Some(walk(&serde_json::from_str::<serde_json::Value>(text).ok()?))
}

/// A TOON mask, read as indentation alone.
///
/// A MASK IS ONLY KEYS. Its values say nothing a mask needs (a key present is
/// a field wanted), so this reads the nesting and ignores everything after the
/// colon, which is also what keeps it from needing a TOON parser.
fn toon_mask(text: &str) -> Mask {
    let mut root = Mask::default();
    let mut stack: Vec<usize> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let depth = (line.len() - trimmed.len()) / 2;
        let key = trimmed
            .split(':')
            .next()
            .unwrap_or_default()
            .trim_end_matches(|character: char| character != '_' && !character.is_alphanumeric())
            .trim();
        if key.is_empty() {
            continue;
        }
        stack.truncate(depth);
        let mut at = &mut root;
        for index in &stack {
            at = &mut at.fields[*index].1;
        }
        at.fields.push((key.to_string(), Mask::default()));
        stack.push(at.fields.len() - 1);
    }
    root
}

#[cfg(test)]
#[path = "recap_document_wire/tests.rs"]
mod tests;
