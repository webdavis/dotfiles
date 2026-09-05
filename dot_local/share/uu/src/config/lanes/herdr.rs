//! `[lanes.herdr]`: the herdr binary to drive, and the plugin roster to
//! refresh.

use crate::config::ConfigError;
use crate::config::schema::{admits, non_empty};

#[derive(Debug, Clone, PartialEq)]
pub struct HerdrLane {
    pub binary: String,
    pub plugins: Vec<Plugin>,
}

/// The herdr command when no key states one.
pub const DEFAULT_HERDR_BINARY: &str = "herdr";

/// One GitHub-sourced herdr plugin: the installed id, and the source to
/// reinstall it from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub repo: String,
}

pub(super) fn parse_herdr_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<HerdrLane, ConfigError> {
    let mut lane = HerdrLane {
        binary: DEFAULT_HERDR_BINARY.to_string(),
        plugins: Vec::new(),
    };
    for (name, setting) in table {
        admits(table_label, "lanes.herdr", &name)?;
        match name.as_str() {
            "binary" => lane.binary = non_empty(table_label, &name, &setting)?,
            "plugins" => lane.plugins = parse_plugins(table_label, &setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

/// `plugins`, a list of `{ id, repo }` tables.
///
/// BOTH FIELDS ARE REQUIRED AND NEITHER MAY BE EMPTY. The refresh is an
/// uninstall by id followed by an install from the repo, so half an entry
/// uninstalls a plugin nothing can put back.
fn parse_plugins(table_label: &str, setting: &toml::Value) -> Result<Vec<Plugin>, ConfigError> {
    let Some(entries) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `plugins` has type `{}`, not a list of plugins",
            setting.type_str()
        )));
    };
    entries
        .iter()
        .map(|entry| {
            let Some(fields) = entry.as_table() else {
                return Err(ConfigError::Invalid(format!(
                    "`{table_label}` key `plugins` holds a `{}`, not a plugin table",
                    entry.type_str()
                )));
            };
            for key in fields.keys() {
                if key != "id" && key != "repo" {
                    return Err(ConfigError::Invalid(format!(
                        "unknown `{table_label}` plugin key `{key}`; a plugin serves id, repo"
                    )));
                }
            }
            let field = |key: &str| {
                fields
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        ConfigError::Invalid(format!(
                            "`{table_label}` plugin entry has no usable `{key}` (missing, empty or \
                             only whitespace), so it names nothing to refresh"
                        ))
                    })
            };
            Ok(Plugin {
                id: field("id")?,
                repo: field("repo")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaneKind;
    use crate::config::probes::{kind, parsed, refusal};

    #[test]
    fn the_herdr_lane_still_parses_with_its_type_written_out() {
        assert_eq!(
            kind(&parsed("[lanes.herdr]\ntype = \"herdr\"\n"), "herdr"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn a_herdr_lane_may_carry_any_name_once_its_type_says_herdr() {
        assert_eq!(
            kind(&parsed("[lanes.mine]\ntype = \"herdr\"\n"), "mine"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn a_lane_block_with_nothing_in_it_is_the_lane_on_with_its_defaults() {
        assert_eq!(
            kind(&parsed("[lanes.herdr]\n"), "herdr"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn the_plugin_roster_is_read_as_id_and_repo_pairs_in_the_order_written() {
        let config = parsed(
            "[lanes.herdr]\n\
             plugins = [\n\
               { id = \"worktrunk\", repo = \"owner/herdr-worktrunk\" },\n\
               { id = \"herdr-bar\", repo = \"other/herdr-bar\" },\n\
             ]\n",
        );
        let Some(LaneKind::Herdr(herdr)) = kind(&config, "herdr") else {
            panic!("expected a herdr lane, got {:?}", config.lanes.get("herdr"));
        };
        assert_eq!(
            herdr.plugins,
            vec![
                Plugin {
                    id: "worktrunk".to_string(),
                    repo: "owner/herdr-worktrunk".to_string(),
                },
                Plugin {
                    id: "herdr-bar".to_string(),
                    repo: "other/herdr-bar".to_string(),
                },
            ]
        );
    }

    #[test]
    fn half_a_plugin_entry_is_refused_because_a_refresh_uninstalls_before_it_installs() {
        for text in [
            "[lanes.herdr]\nplugins = [{ id = \"a\" }]\n",
            "[lanes.herdr]\nplugins = [{ repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"\", repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"\" }]\n",
            // A blank field is the same uninstall with the same nothing to
            // reinstall from, and it reads as a filled-in entry.
            "[lanes.herdr]\nplugins = [{ id = \" \", repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"\t\" }]\n",
        ] {
            let detail = refusal(text);
            assert!(detail.contains("nothing to refresh"), "{detail}");
        }
    }

    #[test]
    fn a_plugin_entry_refuses_a_key_it_does_not_serve() {
        let detail =
            refusal("[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"o/r\", pin = \"v1\" }]\n");
        assert!(
            detail.contains("unknown `lanes.herdr` plugin key `pin`"),
            "{detail}"
        );
    }

    #[test]
    fn a_plugin_list_that_is_not_a_list_of_tables_is_refused_by_name() {
        assert!(
            refusal("[lanes.herdr]\nplugins = \"worktrunk\"\n").contains("not a list of plugins")
        );
        assert!(
            refusal("[lanes.herdr]\nplugins = [\"worktrunk\"]\n").contains("not a plugin table")
        );
    }

    #[test]
    fn a_user_named_lanes_plugin_refusals_name_its_own_table_not_lanes_herdr() {
        // ONE CASE PER REFUSAL IN `parse_plugins`, because each spells the
        // table on its own and restoring the literal in any one of them
        // leaves the other three green.
        for (plugins, says) in [
            ("1", "`lanes.mine` key `plugins` has type `integer`"),
            ("[\"a\"]", "`lanes.mine` key `plugins` holds a `string`"),
            (
                "[{ id = \"a\", repo = \"o/r\", pin = \"v1\" }]",
                "unknown `lanes.mine` plugin key `pin`",
            ),
            (
                "[{ id = \"a\" }]",
                "`lanes.mine` plugin entry has no usable `repo`",
            ),
        ] {
            let detail = refusal(&format!(
                "[lanes.mine]\ntype = \"herdr\"\nplugins = {plugins}\n"
            ));
            assert!(detail.contains(says), "{detail}");
            assert!(!detail.contains("lanes.herdr"), "{detail}");
        }
    }
}
