//! `[lanes.<name>]` with `type = "npm"`: the npm to run.
//!
//! There is no roster key, because `npm update -g` is already every globally
//! installed package.
//!
//! THE PATH IS REQUIRED AND ABSOLUTE, with no default. The lane runs npm with
//! its OWN directory first on PATH so npm's `#!/usr/bin/env node` shebang
//! finds the node beside it, and the directory fnm installs both into lives
//! under the operator's home, which this file can state and a compiled-in
//! default cannot compose. A bare name resolved on whatever PATH uu inherited
//! is the exact mistake the prepend exists to prevent, so it is refused rather
//! than run.

use crate::config::ConfigError;
use crate::config::schema::{absolute, admits};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpmLane {
    pub binary: String,
}

pub(super) fn parse_npm_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<NpmLane, ConfigError> {
    let mut binary = None;
    for (name, setting) in table {
        admits(table_label, "lanes.npm", &name)?;
        match name.as_str() {
            "binary" => binary = Some(absolute(table_label, &name, &setting)?),
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
    Ok(NpmLane { binary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaneKind;
    use crate::config::probes::{kind, parsed, refusal};

    #[test]
    fn an_npm_lane_runs_the_npm_it_was_pointed_at_under_any_name() {
        assert_eq!(
            kind(
                &parsed("[lanes.globals]\ntype = \"npm\"\nbinary = \"/fnm/bin/npm\"\n"),
                "globals"
            ),
            Some(&LaneKind::Npm(NpmLane {
                binary: "/fnm/bin/npm".to_string(),
            }))
        );
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
