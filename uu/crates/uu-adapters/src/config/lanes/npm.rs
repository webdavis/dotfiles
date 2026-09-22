//! `[lanes.<name>]` with `type = "npm"`: the npm to run.
//!
//! `declared` IS A REPORT'S REFERENCE, NOT A ROSTER TO INSTALL FROM. The
//! upgrade is still `npm update -g`, which is already every globally installed
//! package; the list only says which of those the operator meant to have, so
//! the lane can name the rest. Nothing is ever removed, and an absent key
//! leaves the lane doing the upgrade alone.
//!
//! THE PATH IS REQUIRED AND ABSOLUTE, with no default. The lane runs npm with
//! its OWN directory first on PATH so npm's `#!/usr/bin/env node` shebang
//! finds the node beside it, and the directory fnm installs both into lives
//! under the operator's home, which this file can state and a compiled-in
//! default cannot compose. A bare name resolved on whatever PATH uu inherited
//! is the exact mistake the prepend exists to prevent, so it is refused rather
//! than run.

use crate::config::ConfigError;
use crate::config::schema::{absolute, admits_lane, text_list};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpmLane {
    pub(crate) binary: String,
    pub(crate) declared: Option<Vec<String>>,
}

pub(crate) fn parse_npm_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<NpmLane, ConfigError> {
    let mut binary = None;
    let mut declared = None;
    for (name, setting) in table {
        admits_lane(table_label, "npm", NpmLane::KEYS, &name)?;
        match name.as_str() {
            "binary" => binary = Some(absolute(table_label, &name, &setting)?),
            "declared" => declared = Some(text_list(table_label, &name, &setting)?),
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    let binary = binary.ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{table_label}` has no `binary`, so it names no npm to run; state the full path to \
             npm, whose own directory the lane puts first on PATH"
        ))
    })?;
    Ok(NpmLane { binary, declared })
}

impl NpmLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "binary",
        "deadline_secs",
        "declared",
        "escalate_after_runs",
        "type",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{checked_text, refusal, typed};

    #[test]
    fn an_npm_lane_runs_the_npm_it_was_pointed_at_under_any_name() {
        assert_eq!(
            typed::<NpmLane>(
                checked_text("[lanes.globals]\ntype = \"npm\"\nbinary = \"/fnm/bin/npm\"\n"),
                "globals"
            ),
            Some(NpmLane {
                binary: "/fnm/bin/npm".to_string(),
                declared: None,
            })
        );
    }

    #[test]
    fn an_npm_lane_may_declare_the_roster_its_report_is_measured_against() {
        assert_eq!(
            typed::<NpmLane>(
                checked_text(
                    "[lanes.npm]\nbinary = \"/fnm/bin/npm\"\ndeclared = [\"acpx\", \"@scope/cli\"]\n"
                ),
                "npm"
            ),
            Some(NpmLane {
                binary: "/fnm/bin/npm".to_string(),
                declared: Some(vec!["acpx".to_string(), "@scope/cli".to_string()]),
            })
        );
    }

    #[test]
    fn a_declared_roster_that_is_not_a_list_of_names_is_refused_naming_the_key() {
        // A bare string read as a one-name roster would report every other
        // installed package as undeclared, weekly.
        let detail = refusal("[lanes.npm]\nbinary = \"/fnm/bin/npm\"\ndeclared = \"acpx\"\n");
        assert!(detail.contains("`declared`"), "{detail}");
        let blank = refusal("[lanes.npm]\nbinary = \"/fnm/bin/npm\"\ndeclared = [\" \"]\n");
        assert!(blank.contains("`declared`"), "{blank}");
    }

    #[test]
    fn an_npm_lane_that_names_no_binary_is_refused_rather_than_defaulted() {
        // There is no useful compiled-in default: fnm's npm lives under the
        // operator's home, and a bare `npm` off the inherited PATH is the
        // wrong-node bug the lane exists to prevent.
        let detail = refusal("[lanes.npm]\n");
        assert!(detail.contains("`lanes.npm` has no `binary`"), "{detail}");
    }

    #[test]
    fn an_npm_binary_that_is_not_an_absolute_path_is_refused_by_name() {
        for stated in ["npm", "bin/npm", "~/bin/npm"] {
            let detail = refusal(&format!("[lanes.npm]\nbinary = {stated:?}\n"));
            assert!(
                detail.contains("is not an absolute path"),
                "case {stated:?}: {detail}"
            );
        }
    }
}
