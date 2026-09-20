use super::*;

/// The phone's attention marker off `[plugins.phone] marker_file`, or `None`
/// when no key states one.
///
/// READ WHETHER OR NOT THE TABLE IS ARMED, which is the one key under this
/// heading that is: `pns tap` writes the marker and the presence reader reads
/// it, so an operator who switched the card off still taps.
///
/// THE PATH IS JUDGED, NEVER ECHOED. A relative path, another user's `~`, and
/// a control character each name the key and nothing of the value, because a
/// marker path is the operator's own filesystem.
pub(super) fn marker_file(settings: &toml::Table) -> Result<Option<String>, ConfigError> {
    let Some(stated) = settings.get("marker_file") else {
        return Ok(None);
    };
    let path = text(PHONE_TABLE, "marker_file", stated)?;
    if path.is_empty()
        || path.chars().any(char::is_control)
        || !(Path::new(&path).is_absolute() || path.starts_with("~/"))
    {
        return Err(ConfigError::Invalid(format!(
            "`{PHONE_TABLE}` key `marker_file` needs an absolute path or ~/ path without \
             control characters"
        )));
    }
    Ok(Some(path))
}

/// The heading every refusal in this module names.
const PHONE_TABLE: &str = "plugins.phone";

/// The one `[plugins.phone] type` a compiled-in backend answers. VALIDATED
/// AND THEN DISCARDED, the way the router sensor's is: the enum that
/// dispatches between two backends is worth writing the day there are two.
pub const MOSHI_TYPE: &str = "moshi";

/// Whether the mobile table names a backend this binary answers, and the
/// REASON when it does not.
///
/// `phone` IS THE PLUGIN AND `type` IS WHAT IS BEHIND IT, which is why this
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
pub fn phone_backend(settings: &toml::Table) -> Result<(), String> {
    let named = settings
        .get("type")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("no type in [plugins.phone]; the only type is {MOSHI_TYPE:?}"))?;
    if named != MOSHI_TYPE {
        return Err(format!(
            "[plugins.phone] has type {named:?}, which no compiled-in backend answers; \
             the only type is {MOSHI_TYPE:?}"
        ));
    }
    Ok(())
}

/// The moshi token out of the `[plugins.phone]` settings, or None for every
/// way the config can fail to provide one: no `token` key, the wrong type,
/// or an empty value. All of them mean "not set up", never an error.
pub fn moshi_secret(settings: &toml::Table) -> Option<String> {
    let token = settings.get("token")?.as_str()?;
    (!token.is_empty()).then(|| token.to_string())
}

/// The card types whose cards carry an image: every key of
/// `[plugins.phone.image_cards]` whose value is `true`.
///
/// A CARD TYPE IS THE EVENT'S STATE, which is the word its producer sent
/// (`missed` is the card a return moment raises, `failed` a turn that died).
/// pns compiles in no roster of them, so nothing here can refuse a key by
/// name; an unknown one is a card type that never fires rather than a
/// refusal at load, the same trade `[plugins.log.channels]` makes.
///
/// A NON-BOOLEAN IS REFUSED OUT LOUD, the way `card_while_watching` is one
/// level down: reading `"true"` as off leaves the operator having asked for
/// something, not got it, and been told nothing.
pub fn moshi_image_cards(settings: &toml::Table) -> Vec<String> {
    let Some(table) = settings.get("image_cards").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    table
        .iter()
        .filter(|(kind, stated)| match stated.as_bool() {
            Some(on) => on,
            None => {
                eprintln!(
                    "pns: config error ([plugins.phone.image_cards] {kind} is {}, not a boolean); \
                     that card type keeps its text card",
                    stated.type_str()
                );
                false
            }
        })
        .map(|(kind, _)| kind.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{MOSHI_TYPE, moshi_image_cards, moshi_secret, phone_backend};

    // --- which card types carry an image ------------------------------------

    #[test]
    fn only_a_card_type_switched_on_carries_an_image() {
        let settings: toml::Table = "[image_cards]\nmissed = true\ndone = false\n"
            .parse()
            .unwrap();
        assert_eq!(moshi_image_cards(&settings), ["missed"]);
    }

    #[test]
    fn a_table_nobody_wrote_switches_every_card_type_off() {
        // SHIPPED OFF FOR EVERY CARD TYPE is the whole posture of this
        // feature, and absence is how the shipped config states it.
        for settings in ["", "token = \"x\"\n", "[image_cards]\n"] {
            assert!(
                moshi_image_cards(&settings.parse().unwrap()).is_empty(),
                "`{settings}` armed an image card"
            );
        }
    }

    #[test]
    fn a_card_type_whose_value_is_not_a_boolean_keeps_its_text_card() {
        // THE MUTANT THIS PINS: `as_bool().unwrap_or(true)`, which would arm
        // a card type off a typo.
        let settings: toml::Table = "[image_cards]\nmissed = \"true\"\ndone = 1\n"
            .parse()
            .unwrap();
        assert!(moshi_image_cards(&settings).is_empty());
    }

    #[test]
    fn an_image_cards_key_that_is_not_a_table_arms_nothing() {
        let settings: toml::Table = "image_cards = true\n".parse().unwrap();
        assert!(moshi_image_cards(&settings).is_empty());
    }

    // --- the backend the table names ----------------------------------------

    #[test]
    fn the_table_has_to_name_a_backend_and_the_refusal_names_the_key() {
        // NOTHING GUESSES A BACKEND. `phone` is the plugin and `type` is the
        // implementation behind it, so a table that names none is refused
        // rather than read as this one: the day a second backend compiles in,
        // a config that never said which would silently keep whichever arm
        // happened to be first. A non-string and an EMPTY value are the same
        // hole as an absent key, which is the router's own reading.
        for settings in ["", "token = \"tok-1\"\n", "type = 5\n", "type = \"\"\n"] {
            let complaint = phone_backend(&settings.parse().unwrap())
                .expect_err(&format!("case: {settings:?}"));
            assert!(complaint.contains("type"), "names the key: {complaint}");
            assert!(
                complaint.contains("[plugins.phone]"),
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
        let complaint = phone_backend(&"type = \"pushover\"\n".parse().unwrap())
            .expect_err("no backend answers `pushover`");
        assert!(complaint.contains("\"pushover\""), "got: {complaint}");
        assert!(complaint.contains(MOSHI_TYPE), "got: {complaint}");
    }

    #[test]
    fn the_one_compiled_in_type_is_accepted_and_its_token_is_read() {
        // The positive control: a refusal that fired on every table would pass
        // the two tests above and take the phone card away entirely.
        let settings: toml::Table = "type = \"moshi\"\ntoken = \"tok-1\"\n".parse().unwrap();
        assert_eq!(phone_backend(&settings), Ok(()));
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
