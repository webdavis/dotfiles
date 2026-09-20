//! The machine-readable recap: a tree of plain values, and the field mask that
//! narrows it.
//!
//! A TREE OF ITS OWN RATHER THAN `serde_json::Value`, because this crate has
//! no dependencies and because the document is a CONTRACT other tools read.
//! Bob takes it as JSON or as TOON off the same tree, so the shape is decided
//! once here and the two encoders in the adapters only spell it.
//!
//! ORDER IS PART OF THE SHAPE. A map is a vector of pairs rather than a hash,
//! so the document a consumer diffs today has its keys where they were
//! yesterday, and a golden fixture pins bytes rather than a serializer's mood.

/// One value of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Number(u64),
    Flag(bool),
    Absent,
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    /// A map from pairs, which is what every builder in this module writes.
    pub fn map<const N: usize>(pairs: [(&str, Node); N]) -> Node {
        Node::Map(
            pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }
    pub fn text(value: &str) -> Node {
        Node::Text(value.to_string())
    }
    /// A list of plain strings, which is the shape every source section's
    /// `rows` takes.
    pub fn rows(values: &[String]) -> Node {
        Node::List(values.iter().map(|row| Node::Text(row.clone())).collect())
    }
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Map(pairs) => pairs
                .iter()
                .find_map(|(held, value)| (held == key).then_some(value)),
            _ => None,
        }
    }
}

/// The version of the document's shape. A consumer checks it, and a change to
/// the shape bumps it.
pub const SCHEMA: u64 = 1;

/// A field mask: the keys a consumer asked for, nested the way the document
/// is. A leaf carries no children, which means "this field, whole".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mask {
    pub fields: Vec<(String, Mask)>,
}

impl Mask {
    fn of(&self, key: &str) -> Option<&Mask> {
        self.fields
            .iter()
            .find_map(|(held, mask)| (held == key).then_some(mask))
    }
    fn is_leaf(&self) -> bool {
        self.fields.is_empty()
    }
}

/// The key a mask names that the document does not have, or None when every
/// key in it exists.
///
/// A TYPO NEVER SILENTLY EMPTIES A FIELD, which is the whole reason this is
/// checked rather than applied generously: a mask asking for `section` instead
/// of `sections` would otherwise return a document holding the schema and
/// nothing else, and the consumer would read it as a quiet window.
///
/// THE PATH IS DOTTED IN THE REFUSAL, so a key nested three deep is named
/// where it sits rather than by its last word alone.
pub fn unknown_key(document: &Node, mask: &Mask, path: &str) -> Option<String> {
    for (key, nested) in &mask.fields {
        let at = match path.is_empty() {
            true => key.clone(),
            false => format!("{path}.{key}"),
        };
        let Some(value) = probe(document, key) else {
            return Some(at);
        };
        if let Some(found) = unknown_key(value, nested, &at) {
            return Some(found);
        }
    }
    None
}

/// The document narrowed to what the mask names. A key the mask leaves out is
/// absent from the output, and a leaf takes its value whole.
pub fn apply(document: &Node, mask: &Mask) -> Node {
    if mask.is_leaf() {
        return document.clone();
    }
    match document {
        // A LIST IS MASKED ELEMENT BY ELEMENT, so a mask over `sessions`
        // narrows every session rather than only the first.
        Node::List(items) => Node::List(items.iter().map(|item| apply(item, mask)).collect()),
        Node::Map(pairs) => Node::Map(
            pairs
                .iter()
                .filter_map(|(key, value)| {
                    mask.of(key)
                        .map(|nested| (key.clone(), apply(value, nested)))
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The node a key names, looking THROUGH a list so a mask can describe one
/// element of a uniform array rather than every index of it.
fn probe<'tree>(document: &'tree Node, key: &str) -> Option<&'tree Node> {
    match document {
        Node::List(items) => items.iter().find_map(|item| probe(item, key)),
        other => other.get(key),
    }
}

#[cfg(test)]
#[path = "document/tests.rs"]
mod tests;
