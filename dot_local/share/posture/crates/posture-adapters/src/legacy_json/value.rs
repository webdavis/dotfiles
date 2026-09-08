use super::{ProjectionInput, display_number};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use std::fmt;

pub(crate) struct Fields<'a>(pub(crate) Vec<(String, &'a RawValue)>);
impl<'de> Deserialize<'de> for Fields<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = Fields<'de>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object containing launchd fields")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut fields: Vec<(String, &'de RawValue)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, &'de RawValue>()? {
                    if let Some(stored) = fields.iter_mut().find(|(name, _)| name == &key) {
                        stored.1 = value;
                    } else {
                        fields.push((key, value));
                    }
                }
                Ok(Fields(fields))
            }
        }
        deserializer.deserialize_map(Object)
    }
}

pub(crate) fn field(input: &ProjectionInput, fields: &Fields<'_>, name: &str) -> Option<String> {
    let Some((_, value)) = fields.0.iter().find(|(key, _)| key == name) else {
        return Some(String::new());
    };
    // jq serializes the selected row before extracting these fields. NaN becomes null in
    // that round trip, whereas a source label read directly fromjson remains the text null.
    let value = if let Some(number) = input.number(value) {
        if number == "null" {
            String::new()
        } else {
            display_number(number)?
        }
    } else if value.get().starts_with('"') {
        serde_json::from_str(value.get()).ok()?
    } else if matches!(value.get(), "null" | "false") {
        String::new()
    } else {
        rendered(input, value)?
    };
    Some(command_text(value))
}

pub(crate) fn command_text(mut value: String) -> String {
    value.retain(|character| character != '\0');
    value.truncate(value.trim_end_matches('\n').len());
    value
}
enum Part<'a> {
    Value(&'a RawValue, usize),
    Text(String),
}
fn rendered(input: &ProjectionInput, value: &RawValue) -> Option<String> {
    let mut output = String::new();
    let mut pending = vec![Part::Value(value, 0)];
    while let Some(part) = pending.pop() {
        let Part::Value(value, depth) = part else {
            if let Part::Text(text) = part {
                output.push_str(&text);
            }
            continue;
        };
        if let Some(number) = input.number(value) {
            output.push_str(&if number == "null" {
                "null".to_owned()
            } else {
                display_number(number)?
            });
            continue;
        }
        let raw = value.get();
        let (open, close, children): (&str, &str, Vec<(Option<String>, &RawValue)>) =
            match raw.as_bytes().first()? {
                b'[' => (
                    "[",
                    "]",
                    serde_json::from_str::<Vec<&RawValue>>(raw)
                        .ok()?
                        .into_iter()
                        .map(|child| (None, child))
                        .collect(),
                ),
                b'{' => (
                    "{",
                    "}",
                    serde_json::from_str::<Fields<'_>>(raw)
                        .ok()?
                        .0
                        .into_iter()
                        .map(|(key, child)| (Some(key), child))
                        .collect(),
                ),
                b'"' => {
                    let text: String = serde_json::from_str(raw).ok()?;
                    output.push_str(&serde_json::to_string(&text).ok()?);
                    continue;
                }
                _ => {
                    output.push_str(raw);
                    continue;
                }
            };
        output.push_str(open);
        if children.is_empty() {
            output.push_str(close);
            continue;
        }
        output.push('\n');
        pending.push(Part::Text(format!("\n{}{close}", "  ".repeat(depth))));
        for (index, (key, child)) in children.into_iter().enumerate().rev() {
            pending.push(Part::Value(child, depth + 1));
            let mut prefix = "  ".repeat(depth + 1);
            if let Some(key) = key {
                prefix.push_str(&serde_json::to_string(&key).ok()?);
                prefix.push_str(": ");
            }
            pending.push(Part::Text(prefix));
            if index > 0 {
                pending.push(Part::Text(",\n".into()));
            }
        }
    }
    Some(output)
}
