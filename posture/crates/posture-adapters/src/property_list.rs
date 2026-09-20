use plist::Value;
use std::path::Path;

/// A property list parsed in process, in either the binary or the XML wire format.
pub(crate) struct PropertyList(Value);

impl PropertyList {
    /// `None` when the file is missing, unreadable, or not a property list.
    pub(crate) fn read(path: &Path) -> Option<Self> {
        Value::from_file(path).ok().map(Self)
    }

    /// Whether the root dictionary declares `key`, whatever its value.
    pub(crate) fn declares(&self, key: &str) -> bool {
        self.0
            .as_dictionary()
            .is_some_and(|root| root.contains_key(key))
    }

    /// Whether any string value anywhere in the list equals `needle` exactly.
    pub(crate) fn contains_string(&self, needle: &str) -> bool {
        let mut pending = vec![&self.0];
        while let Some(value) = pending.pop() {
            match value {
                Value::String(text) => {
                    if text == needle {
                        return true;
                    }
                }
                Value::Array(values) => pending.extend(values),
                Value::Dictionary(entries) => pending.extend(entries.values()),
                _ => {}
            }
        }
        false
    }

    /// The scalar (string, integer, real, or boolean) at a dot-separated key path.
    pub(crate) fn raw(&self, key_path: &str) -> Option<Vec<u8>> {
        let mut value = &self.0;
        for segment in key_path.split('.') {
            value = match value {
                Value::Dictionary(entries) => entries.get(segment)?,
                Value::Array(values) => values.get(segment.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(match value {
            Value::String(text) => text.clone().into_bytes(),
            Value::Integer(number) => number.to_string().into_bytes(),
            Value::Real(number) => number.to_string().into_bytes(),
            Value::Boolean(flag) => flag.to_string().into_bytes(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
