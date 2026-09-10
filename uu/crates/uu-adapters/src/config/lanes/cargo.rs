//! `[lanes.<name>]` with `type = "cargo"`: the cargo binary to drive, and
//! whether this lane compiles or only reports.
//!
//! REPORTING IS THE DEFAULT because compiling is not. A `cargo install` builds
//! from source and can take minutes per crate on a weekly unattended run, so
//! turning it on is a decision the config states rather than one this lane
//! makes on the operator's behalf.

use crate::config::ConfigError;
use crate::config::schema::{admits_lane, boolean, non_empty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoLane {
    pub(crate) cargo: String,
    pub(crate) compile: bool,
}

/// The cargo command when no key states one, resolved on the running process's
/// own PATH exactly as `DEFAULT_UV_BINARY` is. The shipped config states an
/// absolute path instead, because the weekly job's PATH is the plist's.
pub const DEFAULT_CARGO_BINARY: &str = "cargo";

pub(crate) fn parse_cargo_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<CargoLane, ConfigError> {
    let mut lane = CargoLane {
        cargo: DEFAULT_CARGO_BINARY.to_string(),
        compile: false,
    };
    for (name, setting) in table {
        admits_lane(table_label, "cargo", CargoLane::KEYS, &name)?;
        match name.as_str() {
            "cargo" => lane.cargo = non_empty(table_label, &name, &setting)?,
            "compile" => lane.compile = boolean(table_label, &name, &setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

impl CargoLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "cargo",
        "compile",
        "deadline_secs",
        "escalate_after_runs",
        "type",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{checked_text, refusal, typed};

    #[test]
    fn a_cargo_lane_defaults_to_the_cargo_command_on_the_running_path_and_reports_only() {
        assert_eq!(
            typed::<CargoLane>(checked_text("[lanes.cargo]\n"), "cargo"),
            Some(CargoLane {
                cargo: DEFAULT_CARGO_BINARY.to_string(),
                compile: false,
            })
        );
    }

    #[test]
    fn a_cargo_lane_may_carry_any_name_and_drive_the_binary_it_states() {
        assert_eq!(
            typed::<CargoLane>(
                checked_text(
                    "[lanes.rust-tools]\ntype = \"cargo\"\ncargo = \"/Users/x/.cargo/bin/cargo\"\ncompile = true\n"
                ),
                "rust-tools"
            ),
            Some(CargoLane {
                cargo: "/Users/x/.cargo/bin/cargo".to_string(),
                compile: true,
            })
        );
    }

    #[test]
    fn compile_that_is_not_a_boolean_is_refused_naming_what_was_written() {
        // A string `"true"` reading as false would silently turn compiling off
        // on a machine whose operator believes it is on.
        let why = refusal("[lanes.cargo]\ncompile = \"true\"\n");
        assert!(why.contains("compile"), "{why}");
    }
}
