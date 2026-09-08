mod value;
pub(super) use value::{
    Fields as ProjectionFields, command_text, compact_row, field as projected_field, selected_text,
};

mod number;
pub(super) use number::display_number;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) struct ProjectionInput {
    pub(super) text: String,
    numbers: BTreeMap<usize, String>,
}
impl ProjectionInput {
    pub(super) fn new(raw: &[u8]) -> Option<Self> {
        let text = String::from_utf8_lossy(raw).into_owned();
        let mut bytes = text.as_bytes().to_vec();
        let mut numbers = BTreeMap::new();
        let mut position = 0;
        let mut container_slots = 0usize;
        while position < bytes.len() {
            match bytes[position] {
                b'"' => position = string_end(&mut bytes, position)?,
                b'{' | b'[' => {
                    // The legacy reader checks its 10,000-slot stack before opening a
                    // container. Objects occupy two slots while their values are read.
                    if container_slots >= 10_000 {
                        return None;
                    }
                    container_slots += if bytes[position] == b'{' { 2 } else { 1 };
                    position += 1;
                }
                b'}' | b']' => {
                    container_slots =
                        container_slots.checked_sub(if bytes[position] == b'}' { 2 } else { 1 })?;
                    position += 1;
                }
                byte if byte.is_ascii_whitespace() || b"{}[],:".contains(&byte) => position += 1,
                _ => {
                    let start = position;
                    while position < bytes.len()
                        && !bytes[position].is_ascii_whitespace()
                        && !b"{}[],:\"".contains(&bytes[position])
                    {
                        position += 1;
                    }
                    let token = &text[start..position];
                    if matches!(token, "true" | "false" | "null") {
                        continue;
                    }
                    numbers.insert(start, display_number(token)?);
                    // Keep offsets stable for RawValue projection. Serde still validates the
                    // surrounding grammar; original source bytes never leave their own buffer.
                    bytes[start] = b'0';
                    bytes[start + 1..position].fill(b' ');
                }
            }
        }
        Some(Self {
            text: String::from_utf8(bytes).ok()?,
            numbers,
        })
    }
    pub(super) fn number(&self, value: &RawValue) -> Option<&str> {
        let offset = (value.get().as_ptr() as usize).checked_sub(self.text.as_ptr() as usize)?;
        self.numbers.get(&offset).map(String::as_str)
    }
    pub(super) fn scalar(&self, value: &RawValue) -> Option<String> {
        let token = value.get();
        if let Some(number) = self.number(value) {
            return Some(number.to_owned());
        }
        match token.as_bytes().first()? {
            b'"' => serde_json::from_str(token).ok(),
            b't' if token == "true" => Some("true".into()),
            _ => None,
        }
    }
}
fn string_end(bytes: &mut [u8], start: usize) -> Option<usize> {
    let mut position = start + 1;
    while position < bytes.len() {
        match bytes[position] {
            b'"' => return Some(position + 1),
            b'\\' if bytes.get(position + 1) == Some(&b'u') => {
                let scalar = unicode_escape(bytes, position)?;
                if (0xd800..=0xdbff).contains(&scalar) {
                    let low = unicode_escape(bytes, position + 6)?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return None;
                    }
                    position += 12;
                } else {
                    // jq accepts an isolated low surrogate as a replacement character.
                    if (0xdc00..=0xdfff).contains(&scalar) {
                        bytes[position..position + 6].copy_from_slice(b"\\ufffd");
                    }
                    position += 6;
                }
            }
            b'\\' => position += 2,
            _ => position += 1,
        }
    }
    None
}
fn unicode_escape(bytes: &[u8], position: usize) -> Option<u16> {
    if bytes.get(position..position + 2)? != b"\\u" {
        return None;
    }
    let digits = std::str::from_utf8(bytes.get(position + 2..position + 6)?).ok()?;
    u16::from_str_radix(digits, 16).ok()
}
