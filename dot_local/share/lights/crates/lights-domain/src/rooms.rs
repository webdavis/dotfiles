use crate::ValueError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomName(String);
impl RoomName {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(ValueError("invalid room name"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct Aliases(BTreeMap<String, RoomName>);
impl Aliases {
    pub fn new(values: BTreeMap<String, RoomName>) -> Self {
        Self(values)
    }
    pub fn resolve(&self, value: &str) -> Result<RoomName, ValueError> {
        self.0
            .get(value)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| RoomName::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_resolve_and_other_names_pass_through() {
        let aliases = Aliases::new(BTreeMap::from([(
            "studio".into(),
            RoomName::new("3F - Studio").unwrap(),
        )]));
        assert_eq!(aliases.resolve("studio").unwrap().as_str(), "3F - Studio");
        assert_eq!(
            aliases.resolve("Custom Room").unwrap().as_str(),
            "Custom Room"
        );
        assert!(aliases.resolve("").is_err());
        assert!(aliases.resolve("bad\nroom").is_err());
    }
}
