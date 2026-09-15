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
    /// Every room the table names, in alias order and without repeating a room
    /// two aliases share.
    pub fn rooms(&self) -> Vec<RoomName> {
        let mut rooms: Vec<RoomName> = Vec::new();
        for room in self.0.values() {
            if !rooms.contains(room) {
                rooms.push(room.clone());
            }
        }
        rooms
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
    #[test]
    fn rooms_are_listed_once_each_however_many_aliases_reach_them() {
        let studio = RoomName::new("3F - Studio").unwrap();
        let aliases = Aliases::new(BTreeMap::from([
            ("studio".into(), studio.clone()),
            ("desk".into(), studio),
            ("kitchen".into(), RoomName::new("2F - Kitchen").unwrap()),
        ]));
        assert_eq!(
            aliases
                .rooms()
                .iter()
                .map(RoomName::as_str)
                .collect::<Vec<_>>(),
            ["3F - Studio", "2F - Kitchen"]
        );
    }
}
