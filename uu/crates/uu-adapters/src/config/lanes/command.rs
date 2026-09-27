//! `[lane.<name>]` with no `type`, or any lane block with `type = "command"`:
//! the PRODUCER API.
//!
//! `command[0]` is the program and `command[1..]` its arguments. The lane's
//! NAME is the operator's own choice; nothing here constrains it, which is the
//! whole point of the producer API.

use crate::config::ConfigError;
use crate::config::schema::admits_lane;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandLane {
    pub(crate) run: Vec<String>,
}

/// `command` is required; everything else `admits` already refused.
///
/// `admits` RUNS FIRST, inside this same loop, before the after-loop check
/// below for a missing `command`. A block that names an unknown key AND no
/// `command` (`[lane.mine]\ntype = "command"\nbogus = 1`) is refused for the key
/// it misspelled, not for the command it never got to declare: the operator fixes
/// one problem at a time, and "unknown key" is the more specific diagnosis.
pub(crate) fn parse_command_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<CommandLane, ConfigError> {
    let mut command = None;
    for (name, setting) in table {
        if name == "run" {
            return Err(ConfigError::Invalid(format!(
                "`{table_label}` key `run` is now `command`"
            )));
        }
        admits_lane(table_label, "command", CommandLane::KEYS, &name)?;
        match name.as_str() {
            "command" => command = Some(parse_argv(table_label, "command", &setting)?),
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    let run = command.ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{table_label}` has no `command`, so it names nothing to run"
        ))
    })?;
    Ok(CommandLane { run })
}

/// `command`: a non-empty list of non-blank strings. `command[0]` is the program
/// that gets executed and `command[1..]` its arguments, so a missing, wrongly-typed,
/// empty or blank entry each names nothing runnable and is refused by name.
pub(super) fn parse_argv(
    table_label: &str,
    key: &str,
    setting: &toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let Some(entries) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `{key}` has type `{}`, not a list",
            setting.type_str()
        )));
    };
    if entries.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `{key}` is empty, so it names nothing to run"
        )));
    }
    entries
        .iter()
        .map(|entry| match entry.as_str() {
            Some(word) if word.trim().is_empty() => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `{key}` holds a blank entry, so it names nothing to run"
            ))),
            Some(word) => Ok(word.to_string()),
            None => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `{key}` holds a `{}`, not a string",
                entry.type_str()
            ))),
        })
        .collect()
}

impl CommandLane {
    pub(crate) const KEYS: &'static [&'static str] =
        &["command", "deadline_secs", "escalate_after_runs", "type"];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{checked_text, parsed, refusal, typed};

    #[test]
    fn a_command_lane_without_command_is_refused_because_it_names_nothing_to_run() {
        let detail = refusal("[lane.command]\n");
        assert!(detail.contains("has no `command`"), "{detail}");
        assert!(detail.contains("names nothing to run"), "{detail}");
    }

    #[test]
    fn a_command_that_is_empty_not_a_list_or_holds_a_blank_is_refused_by_name() {
        for (text, expect) in [
            ("[lane.command]\ncommand = []\n", "is empty"),
            ("[lane.command]\ncommand = \"x\"\n", "not a list"),
            ("[lane.command]\ncommand = [1]\n", "not a string"),
            ("[lane.command]\ncommand = [\"\"]\n", "holds a blank entry"),
            // Whitespace is blank too: it reads as a filled-in entry and
            // names nothing an exec can find.
            ("[lane.command]\ncommand = [\" \"]\n", "holds a blank entry"),
            // A VALID command[0] must not stop the check: a mutant that
            // validates only the first entry passes every case above.
            (
                "[lane.command]\ncommand = [\"ok\", \"\"]\n",
                "holds a blank entry",
            ),
        ] {
            let detail = refusal(text);
            assert!(detail.contains(expect), "case {text:?}: {detail}");
        }
    }

    #[test]
    fn a_command_lane_reads_command_as_the_program_and_its_arguments() {
        let config =
            checked_text("[lane.mine]\ntype = \"command\"\ncommand = [\"/bin/x\", \"--yes\"]\n");
        assert_eq!(
            typed::<CommandLane>(config, "mine"),
            Some(CommandLane {
                run: vec!["/bin/x".to_string(), "--yes".to_string()],
            })
        );
        // A second way `type` could be ignored: a herdr-only key on a command
        // block must still be refused.
        let detail =
            refusal("[lane.mine]\ntype = \"command\"\ncommand = [\"x\"]\nbinary = \"y\"\n");
        assert!(
            detail.contains("unknown `lane.mine` key `binary`"),
            "{detail}"
        );
    }

    #[test]
    fn a_lane_block_with_no_type_runs_its_command_as_the_program_and_its_arguments() {
        let config = parsed("[lane.mine]\ncommand = [\"/fixture/updater\", \"--yes\"]\n");
        let lane = &config.lanes["mine"];
        assert_eq!(lane.type_name(), "command");
        let expected = CommandLane {
            run: vec!["/fixture/updater".to_string(), "--yes".to_string()],
        };
        assert_eq!(format!("{:?}", lane.adapter), format!("{expected:?}"));
    }

    #[test]
    fn a_command_lane_with_run_is_refused_saying_it_is_now_command() {
        let detail = refusal("[lane.mine]\nrun = [\"/fixture/updater\"]\n");
        assert!(
            detail.contains("`lane.mine` key `run` is now `command`"),
            "{detail}"
        );
    }

    #[test]
    fn a_command_lane_with_a_bogus_key_is_refused_as_unknown_before_the_command_check() {
        let detail = refusal("[lane.command]\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lane.command` key `bogus`"),
            "{detail}"
        );
        assert!(!detail.contains("names nothing to run"), "{detail}");
    }
}
