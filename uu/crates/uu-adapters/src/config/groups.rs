use std::collections::BTreeMap;

use super::ConfigError;
use super::schema::{table_of, text_list};

pub(super) fn parse_groups(
    value: toml::Value,
) -> Result<BTreeMap<String, Vec<String>>, ConfigError> {
    let mut groups = BTreeMap::new();
    for (name, block) in table_of("group", value)? {
        let table_label = format!("group.{name}");
        let table = table_of(&table_label, block)?;
        if let Some(setting) = table.get("lanes") {
            groups.insert(name, text_list(&table_label, "lanes", setting)?);
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use crate::config::probes::parsed;

    const LANE_TABLE: &str = "[lane.first]\ncommand = [\"/fixture/first\"]\n\n\
                              [lane.second]\ncommand = [\"/fixture/second\"]\n\n";

    #[test]
    fn a_group_keeps_its_lanes_in_the_order_the_file_lists_them() {
        let config = parsed(&format!(
            "{LANE_TABLE}[group.both]\nlanes = [\"second\", \"first\"]\n"
        ));
        assert_eq!(config.groups["both"], ["second", "first"]);
    }
}
