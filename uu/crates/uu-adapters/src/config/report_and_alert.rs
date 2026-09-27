use super::ConfigError;
use super::lanes::parse_argv;
use super::schema::{admits, table_of};

pub(super) fn parse_command_block(
    block: &str,
    value: toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let table = table_of(block, value)?;
    for key in table.keys() {
        admits(block, block, key)?;
    }
    let command = table.get("command").ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{block}` has no `command`, so it names nothing to run"
        ))
    })?;
    parse_argv(block, "command", command)
}

#[cfg(test)]
mod tests {
    use crate::config::probes::{parsed, refusal};

    #[test]
    fn a_report_block_and_an_alert_block_each_read_their_command_as_the_program_and_its_arguments()
    {
        let config = parsed(
            "[report]\ncommand = [\"/fixture/report\", \"--quiet\"]\n\n\
             [alert]\ncommand = [\"/fixture/alert\"]\n",
        );
        assert_eq!(
            config.report,
            Some(vec!["/fixture/report".to_string(), "--quiet".to_string()])
        );
        assert_eq!(config.alert, Some(vec!["/fixture/alert".to_string()]));
        let config = parsed("");
        assert_eq!(config.report, None);
        assert_eq!(config.alert, None);
    }

    #[test]
    fn a_report_or_alert_block_that_names_nothing_to_run_or_an_unknown_key_is_refused_by_name() {
        for block in ["report", "alert"] {
            for (text, expect) in [
                (
                    format!("[{block}]\n"),
                    format!("`{block}` has no `command`, so it names nothing to run"),
                ),
                (
                    format!("[{block}]\ncommand = []\n"),
                    format!("`{block}` key `command` is empty"),
                ),
                (
                    format!("[{block}]\ncommand = [\" \"]\n"),
                    format!("`{block}` key `command` holds a blank entry"),
                ),
                (
                    format!("[{block}]\ncommand = \"/fixture/x\"\n"),
                    format!("`{block}` key `command` has type `string`, not a list"),
                ),
                (
                    format!(
                        "[{block}]\ncommand = [\"/fixture/x\"]\ndry_run_command = [\"/fixture/y\"]\n"
                    ),
                    format!("unknown `{block}` key `dry_run_command`; the table serves command"),
                ),
                (
                    format!("{block} = 1\n"),
                    format!("`{block}` has type `integer`, not a table"),
                ),
            ] {
                let detail = refusal(&text);
                assert!(detail.contains(&expect), "case {text:?}: {detail}");
            }
        }
    }
}
