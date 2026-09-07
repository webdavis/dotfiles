/// The one `[plugins.mobile] type` a compiled-in backend answers. VALIDATED
/// AND THEN DISCARDED, the way the router sensor's is: the enum that
/// dispatches between two backends is worth writing the day there are two.
pub const MOSHI_TYPE: &str = "moshi";

/// Whether the mobile table names a backend this binary answers, and the
/// REASON when it does not.
///
/// `mobile` IS THE PLUGIN AND `type` IS WHAT IS BEHIND IT, which is why this
/// is settled before anything under the table is read: every setting there
/// belongs to whichever backend the table names, and a table that names none
/// must not be read as this one. The day a second backend compiles in, a
/// config that never said which would otherwise keep whichever arm happened to
/// be written first, silently.
///
/// PRESENT BUT EMPTY IS THE KEY LEFT BLANK, the same hole as absent and the
/// same reading `home::router_settings` gives its own `type`. THE TWO ARE THE
/// SAME QUESTION ASKED OF TWO TABLES and they are worded to match on purpose:
/// name the table, quote what was written, name the one type that answers.
/// Reword one and reword the other, or the rename that gave both tables one
/// word leaves them two sentences.
///
/// THE REASON CARRIES NO PREFIX AND NO VERDICT. It is wrapped twice, by
/// `refused_backend_line` for the leg that will not be dispatched and by the
/// composition root for its one line on stderr, so one fault has one wording
/// wherever it is said.
///
/// IT IS RETURNED, NOT PRINTED, exactly as `stale_alert_channel`'s complaint
/// is: this stays a value function, and the composition root is where a
/// warning becomes a line.
pub fn mobile_backend(settings: &toml::Table) -> Result<(), String> {
    let named = settings
        .get("type")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("no type in [plugins.mobile]; the only type is {MOSHI_TYPE:?}"))?;
    if named != MOSHI_TYPE {
        return Err(format!(
            "[plugins.mobile] has type {named:?}, which no compiled-in backend answers; \
             the only type is {MOSHI_TYPE:?}"
        ));
    }
    Ok(())
}

/// The moshi token out of the `[plugins.mobile]` settings, or None for every
/// way the config can fail to provide one: no `token` key, the wrong type,
/// or an empty value. All of them mean "not set up", never an error.
pub fn moshi_secret(settings: &toml::Table) -> Option<String> {
    let token = settings.get("token")?.as_str()?;
    (!token.is_empty()).then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::{MOSHI_TYPE, mobile_backend, moshi_secret};

    // --- the backend the table names ----------------------------------------

    #[test]
    fn the_table_has_to_name_a_backend_and_the_refusal_names_the_key() {
        // NOTHING GUESSES A BACKEND. `mobile` is the plugin and `type` is the
        // implementation behind it, so a table that names none is refused
        // rather than read as this one: the day a second backend compiles in,
        // a config that never said which would silently keep whichever arm
        // happened to be first. A non-string and an EMPTY value are the same
        // hole as an absent key, which is the router's own reading.
        for settings in ["", "token = \"tok-1\"\n", "type = 5\n", "type = \"\"\n"] {
            let complaint = mobile_backend(&settings.parse().unwrap())
                .expect_err(&format!("case: {settings:?}"));
            assert!(complaint.contains("type"), "names the key: {complaint}");
            assert!(
                complaint.contains("[plugins.mobile]"),
                "and the table: {complaint}"
            );
            assert!(
                complaint.contains(MOSHI_TYPE),
                "and the one type that answers: {complaint}"
            );
        }
    }

    #[test]
    fn a_type_no_compiled_in_backend_answers_is_refused_quoting_it() {
        let complaint = mobile_backend(&"type = \"pushover\"\n".parse().unwrap())
            .expect_err("no backend answers `pushover`");
        assert!(complaint.contains("\"pushover\""), "got: {complaint}");
        assert!(complaint.contains(MOSHI_TYPE), "got: {complaint}");
    }

    #[test]
    fn the_one_compiled_in_type_is_accepted_and_its_token_is_read() {
        // The positive control: a refusal that fired on every table would pass
        // the two tests above and take the phone card away entirely.
        let settings: toml::Table = "type = \"moshi\"\ntoken = \"tok-1\"\n".parse().unwrap();
        assert_eq!(mobile_backend(&settings), Ok(()));
        assert_eq!(moshi_secret(&settings), Some("tok-1".to_string()));
    }

    // --- the secret ---------------------------------------------------------

    #[test]
    fn the_secret_is_the_non_empty_token_setting() {
        assert_eq!(
            moshi_secret(&"token = \"tok-1\"\nother = \"x\"\n".parse().unwrap()),
            Some("tok-1".to_string())
        );
    }

    #[test]
    fn every_way_the_settings_can_fail_to_provide_a_token_reads_not_set_up() {
        for settings in ["", "other = \"x\"\n", "token = \"\"\n", "token = 42\n"] {
            assert_eq!(
                moshi_secret(&settings.parse().unwrap()),
                None,
                "case: {settings:?}"
            );
        }
    }
}
