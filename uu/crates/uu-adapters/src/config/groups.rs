use std::collections::BTreeMap;

use super::schema::{table_of, text_list};
use super::{ConfigError, Lanes};

pub(super) fn parse_groups(
    value: toml::Value,
    lanes: &Lanes,
) -> Result<BTreeMap<String, Vec<String>>, ConfigError> {
    let mut groups = BTreeMap::new();
    for (name, block) in table_of("group", value)? {
        let table_label = format!("group.{name}");
        let table = table_of(&table_label, block)?;
        if let Some(setting) = table.get("lanes") {
            let members = text_list(&table_label, "lanes", setting)?;
            if let Some(unknown) = members.iter().find(|member| !lanes.contains_key(*member)) {
                return Err(ConfigError::Invalid(format!(
                    "group `{name}` lists `{unknown}`, which is no lane in this file"
                )));
            }
            groups.insert(name, members);
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use crate::config::probes::{parsed, refusal};

    const LANE_TABLE: &str = "[lane.first]\ncommand = [\"/fixture/first\"]\n\n\
                              [lane.second]\ncommand = [\"/fixture/second\"]\n\n";
    const LANES_TABLE: &str = "[lanes.first]\ntype = \"command\"\nrun = [\"/fixture/first\"]\n\n\
                               [lanes.second]\ntype = \"command\"\nrun = [\"/fixture/second\"]\n\n";

    #[test]
    fn a_group_listing_a_name_no_lane_block_declares_is_refused_naming_the_group_and_the_name() {
        for lanes in [LANE_TABLE, LANES_TABLE] {
            let detail = refusal(&format!(
                "{lanes}[group.both]\nlanes = [\"first\", \"third\"]\n"
            ));
            assert!(
                detail.contains("group `both` lists `third`, which is no lane in this file"),
                "{detail}"
            );
        }
    }

    #[test]
    fn a_group_keeps_its_lanes_in_the_order_the_file_lists_them() {
        let config = parsed(&format!(
            "{LANE_TABLE}[group.both]\nlanes = [\"second\", \"first\"]\n"
        ));
        assert_eq!(config.groups["both"], ["second", "first"]);
    }
}
