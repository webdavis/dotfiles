//! `[lanes.<name>]` with `type = "uv"`: the uv binary to drive.
//!
//! There is no roster key, because `uv tool upgrade --all` is already every
//! tool uv installed.

use crate::config::ConfigError;
use crate::config::schema::{admits_lane, non_empty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UvLane {
    pub(crate) binary: String,
}

/// The uv command when no key states one, resolved on the running process's
/// own PATH exactly as `DEFAULT_HERDR_BINARY` is. The shipped config states an
/// absolute path instead, because the weekly job's PATH is the plist's.
pub const DEFAULT_UV_BINARY: &str = "uv";

pub(crate) fn parse_uv_lane(table_label: &str, table: toml::Table) -> Result<UvLane, ConfigError> {
    let mut lane = UvLane {
        binary: DEFAULT_UV_BINARY.to_string(),
    };
    for (name, setting) in table {
        admits_lane(table_label, "uv", UvLane::KEYS, &name)?;
        match name.as_str() {
            "binary" => lane.binary = non_empty(table_label, &name, &setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

impl UvLane {
    pub(crate) const KEYS: &'static [&'static str] =
        &["binary", "deadline_secs", "escalate_after_runs", "type"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{checked_text, typed};

    #[test]
    fn a_uv_lane_defaults_to_the_uv_command_on_the_running_path() {
        assert_eq!(
            typed::<UvLane>(checked_text("[lanes.uv]\n"), "uv"),
            Some(UvLane {
                binary: DEFAULT_UV_BINARY.to_string(),
            })
        );
    }

    #[test]
    fn a_uv_lane_may_carry_any_name_and_drive_the_binary_it_states() {
        assert_eq!(
            typed::<UvLane>(
                checked_text("[lanes.tools]\ntype = \"uv\"\nbinary = \"/opt/homebrew/bin/uv\"\n"),
                "tools"
            ),
            Some(UvLane {
                binary: "/opt/homebrew/bin/uv".to_string(),
            })
        );
    }
}
