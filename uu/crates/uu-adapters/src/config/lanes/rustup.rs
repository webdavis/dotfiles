//! `[lanes.<name>]` with `type = "rustup"`: the rustup binary to drive.
//!
//! There is no roster key, because `rustup update` is already every toolchain
//! this machine installed.

use crate::config::ConfigError;
use crate::config::schema::{admits_lane, non_empty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustupLane {
    pub(crate) rustup: String,
}

/// The rustup command when no key states one, resolved on the running
/// process's own PATH exactly as `DEFAULT_UV_BINARY` is. The shipped config
/// states an absolute path instead, because the weekly job's PATH is the
/// plist's.
pub const DEFAULT_RUSTUP_BINARY: &str = "rustup";

pub(crate) fn parse_rustup_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<RustupLane, ConfigError> {
    let mut lane = RustupLane {
        rustup: DEFAULT_RUSTUP_BINARY.to_string(),
    };
    for (name, setting) in table {
        admits_lane(table_label, "rustup", RustupLane::KEYS, &name)?;
        match name.as_str() {
            "rustup" => lane.rustup = non_empty(table_label, &name, &setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

impl RustupLane {
    pub(crate) const KEYS: &'static [&'static str] =
        &["deadline_secs", "escalate_after_runs", "rustup", "type"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{checked_text, refusal, typed};

    #[test]
    fn a_rustup_lane_defaults_to_the_rustup_command_on_the_running_path() {
        assert_eq!(
            typed::<RustupLane>(checked_text("[lanes.rustup]\n"), "rustup"),
            Some(RustupLane {
                rustup: DEFAULT_RUSTUP_BINARY.to_string(),
            })
        );
    }

    #[test]
    fn a_rustup_lane_may_carry_any_name_and_drive_the_binary_it_states() {
        assert_eq!(
            typed::<RustupLane>(
                checked_text(
                    "[lanes.toolchains]\ntype = \"rustup\"\nrustup = \"/Users/x/.cargo/bin/rustup\"\n"
                ),
                "toolchains"
            ),
            Some(RustupLane {
                rustup: "/Users/x/.cargo/bin/rustup".to_string(),
            })
        );
    }

    #[test]
    fn an_empty_rustup_path_is_refused_rather_than_read_as_the_default() {
        let why = refusal("[lanes.rustup]\nrustup = \"\"\n");
        assert!(why.contains("rustup"), "{why}");
    }
}
