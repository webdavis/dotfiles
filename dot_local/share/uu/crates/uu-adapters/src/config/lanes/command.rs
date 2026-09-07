//! `[lanes.<name>]` with `type = "command"`: the PRODUCER API.
//!
//! `run[0]` is the program and `run[1..]` its arguments. The lane's NAME is
//! the operator's own choice; nothing here constrains it, which is the whole
//! point of the producer API.

use crate::config::ConfigError;
use crate::config::schema::admits;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandLane {
    pub run: Vec<String>,
}

/// `run` is required; everything else `admits` already refused.
///
/// `admits` RUNS FIRST, inside this same loop, before the after-loop check
/// below for a missing `run`. A block that names an unknown key AND no `run`
/// (`[lanes.mine]\ntype = "command"\nbogus = 1`) is refused for the key it
/// misspelled, not for the run it never got to declare: the operator fixes
/// one problem at a time, and "unknown key" is the more specific diagnosis.
pub(super) fn parse_command_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<CommandLane, ConfigError> {
    let mut run = None;
    for (name, setting) in table {
        admits(table_label, "lanes.command", &name)?;
        match name.as_str() {
            "run" => run = Some(parse_run(table_label, &setting)?),
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    let run = run.ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{table_label}` has no `run`, so it names nothing to run"
        ))
    })?;
    Ok(CommandLane { run })
}

/// `run`: a non-empty list of non-blank strings. `run[0]` is the program that
/// gets executed and `run[1..]` its arguments, so a missing, wrongly-typed,
/// empty or blank entry each names nothing runnable and is refused by name.
fn parse_run(table_label: &str, setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let Some(entries) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `run` has type `{}`, not a list",
            setting.type_str()
        )));
    };
    if entries.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `run` is empty, so it names nothing to run"
        )));
    }
    entries
        .iter()
        .map(|entry| match entry.as_str() {
            Some(word) if word.trim().is_empty() => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `run` holds a blank entry, so it names nothing to run"
            ))),
            Some(word) => Ok(word.to_string()),
            None => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `run` holds a `{}`, not a string",
                entry.type_str()
            ))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaneKind;
    use crate::config::probes::{kind, parsed, refusal};

    #[test]
    fn a_command_lane_without_run_is_refused_because_it_names_nothing_to_run() {
        let detail = refusal("[lanes.command]\n");
        assert!(detail.contains("has no `run`"), "{detail}");
        assert!(detail.contains("names nothing to run"), "{detail}");
    }

    #[test]
    fn a_run_that_is_empty_not_a_list_or_holds_a_blank_is_refused_by_name() {
        for (text, expect) in [
            ("[lanes.command]\nrun = []\n", "is empty"),
            ("[lanes.command]\nrun = \"x\"\n", "not a list"),
            ("[lanes.command]\nrun = [1]\n", "not a string"),
            ("[lanes.command]\nrun = [\"\"]\n", "holds a blank entry"),
            // Whitespace is blank too: it reads as a filled-in entry and
            // names nothing an exec can find.
            ("[lanes.command]\nrun = [\" \"]\n", "holds a blank entry"),
            // A VALID run[0] must not stop the check: a mutant that
            // validates only the first entry passes every case above.
            (
                "[lanes.command]\nrun = [\"ok\", \"\"]\n",
                "holds a blank entry",
            ),
        ] {
            let detail = refusal(text);
            assert!(detail.contains(expect), "case {text:?}: {detail}");
        }
    }

    #[test]
    fn a_command_lane_reads_run_as_the_program_and_its_arguments() {
        let config = parsed("[lanes.mine]\ntype = \"command\"\nrun = [\"/bin/x\", \"--yes\"]\n");
        assert_eq!(
            kind(&config, "mine"),
            Some(&LaneKind::Command(CommandLane {
                run: vec!["/bin/x".to_string(), "--yes".to_string()],
            }))
        );
        // A second way `type` could be ignored: a herdr-only key on a command
        // block must still be refused.
        let detail = refusal("[lanes.mine]\ntype = \"command\"\nrun = [\"x\"]\nbinary = \"y\"\n");
        assert!(
            detail.contains("unknown `lanes.mine` key `binary`"),
            "{detail}"
        );
    }

    #[test]
    fn a_command_lane_with_a_bogus_key_is_refused_as_unknown_before_the_run_check() {
        let detail = refusal("[lanes.command]\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lanes.command` key `bogus`"),
            "{detail}"
        );
        assert!(!detail.contains("names nothing to run"), "{detail}");
    }
}
