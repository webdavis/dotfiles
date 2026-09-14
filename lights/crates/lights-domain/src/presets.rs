use crate::RoomName;
use std::collections::BTreeMap;

/// What a preset asks of one room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetTarget {
    Scene(String),
    Off,
}

/// One room and what the preset wants it doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetStep {
    pub room: RoomName,
    pub target: PresetTarget,
}

/// The configured presets, each an ORDERED plan. The order is the operator's:
/// a preset is applied step by step as written, so a map from room to scene
/// would not do, because a table loses the sequence the operator wrote.
#[derive(Debug, Default)]
pub struct Presets(BTreeMap<String, Vec<PresetStep>>);

impl Presets {
    pub fn new(values: BTreeMap<String, Vec<PresetStep>>) -> Self {
        Self(values)
    }
    pub fn plan(&self, name: &str) -> Option<&[PresetStep]> {
        self.0.get(name).map(Vec::as_slice)
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_listed_by_name_and_fetched_by_name() {
        let step = PresetStep {
            room: RoomName::new("Studio").unwrap(),
            target: PresetTarget::Off,
        };
        let presets = Presets::new(BTreeMap::from([
            ("morning".into(), vec![step.clone()]),
            ("evening".into(), vec![step.clone()]),
        ]));
        assert_eq!(presets.names().collect::<Vec<_>>(), ["evening", "morning"]);
        assert_eq!(presets.plan("morning"), Some(&[step][..]));
        assert_eq!(presets.plan("absent"), None);
    }
}
