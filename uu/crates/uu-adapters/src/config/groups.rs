use std::collections::BTreeMap;

use super::schema::{table_of, text_list};
use super::{ConfigError, Lanes};

pub(super) fn parse_groups(
    value: toml::Value,
    lanes: &Lanes,
) -> Result<BTreeMap<String, Vec<String>>, ConfigError> {
    let mut groups = BTreeMap::new();
    for (name, block) in table_of("group", value)? {
        if lanes.contains_key(&name) {
            return Err(ConfigError::Invalid(format!(
                "group `{name}` has the same name as a lane, so `uu run {name}` could mean \
                 either; rename one of them"
            )));
        }
        let table_label = format!("group.{name}");
        let members = parse_members(&table_label, table_of(&table_label, block)?)?;
        if let Some(unknown) = members.iter().find(|member| !lanes.contains_key(*member)) {
            return Err(ConfigError::Invalid(format!(
                "group `{name}` lists `{unknown}`, which is no lane in this file"
            )));
        }
        groups.insert(name, members);
    }
    Ok(groups)
}

fn parse_members(table_label: &str, table: toml::Table) -> Result<Vec<String>, ConfigError> {
    let setting = table.get("lanes").ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{table_label}` has no `lanes`, so it names no lane to run"
        ))
    })?;
    let members = text_list(table_label, "lanes", setting)?;
    if members.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `lanes` is empty, so it names no lane to run"
        )));
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use crate::config::probes::{parsed, refusal};

    const LANE_TABLE: &str = "[lane.first]\ncommand = [\"/fixture/first\"]\n\n\
                              [lane.second]\ncommand = [\"/fixture/second\"]\n\n";
    const LANES_TABLE: &str = "[lanes.first]\ntype = \"command\"\nrun = [\"/fixture/first\"]\n\n\
                               [lanes.second]\ntype = \"command\"\nrun = [\"/fixture/second\"]\n\n";

    #[test]
    fn a_group_block_that_is_not_a_non_empty_list_of_lane_names_is_refused_by_name() {
        for (block, expect) in [
            ("[group.both]\n", "`group.both` has no `lanes`"),
            (
                "[group.both]\nlanes = []\n",
                "`group.both` key `lanes` is empty",
            ),
            (
                "[group.both]\nlanes = \"first\"\n",
                "`group.both` key `lanes` has type `string`, not a list of names",
            ),
            (
                "[group.both]\nlanes = [1]\n",
                "`group.both` key `lanes` has type `integer`, not a string",
            ),
            (
                "[group.both]\nlanes = [\" \"]\n",
                "`group.both` key `lanes` is empty or only whitespace",
            ),
            (
                "[group]\nboth = 1\n",
                "`group.both` has type `integer`, not a table",
            ),
            ("group = 1\n", "`group` has type `integer`, not a table"),
        ] {
            let detail = refusal(&format!("{block}\n{LANE_TABLE}"));
            assert!(detail.contains(expect), "case {block:?}: {detail}");
        }
    }

    #[test]
    fn a_group_that_shares_its_name_with_a_lane_is_refused_naming_it() {
        for lanes in [LANE_TABLE, LANES_TABLE] {
            let detail = refusal(&format!("{lanes}[group.first]\nlanes = [\"second\"]\n"));
            assert!(
                detail.contains("group `first` has the same name as a lane"),
                "{detail}"
            );
        }
    }

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
