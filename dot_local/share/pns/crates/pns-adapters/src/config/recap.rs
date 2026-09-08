use super::*;

/// The most any summarizer may be given. ONE HOUR, which is fifteen times the
/// default, so no honest backend on any machine meets it; see `seconds` for the
/// two failures that live past it.
pub(super) const MAX_SUMMARIZER_DEADLINE_SECS: u64 = 3600;

/// `[recap]`'s switches, each starting at its default and moved only by a key
/// that states it.
pub(super) fn parse_recap(value: toml::Value) -> Result<Recap, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`recap` is not a table".to_string()));
    };
    let mut recap = Recap::default();
    // ONE ARM PER KEY, and the three that are not booleans read through a
    // function of their own, so each refusal sits with the shape it judges.
    for (key, setting) in table {
        // BELT AND BRACES HERE, DELIBERATELY, and this is the first of the five
        // top-level tables it reads that way. The `_` arm below refuses the
        // same key with the same sentence for as long as the roster and the
        // arms AGREE, so removing this line changes nothing observable today
        // and a mutation of it survives the suite. Its whole effect is what
        // happens when the two stop agreeing: a key added to an arm and not to
        // the roster stops working at its own feature test instead of quietly
        // working while every refusal listing omits it. Do not delete the five
        // as redundant; the plugin tables have no `_` arm at all, and there the
        // gate is the only check.
        admits_flat("recap", &key)?;
        match key.as_str() {
            "min_events" => recap.min_events = threshold(&setting)?,
            "repos" => recap.repos = repositories(&setting)?,
            "review_notes" => recap.review_notes = Some(note_glob(&setting)?),
            "summarizer" => recap.summarizer = Some(argv(&setting)?),
            "summarizer_deadline_secs" => recap.summarizer_deadline_secs = seconds(&setting)?,
            "replay_card" => recap.replay_card = flag(&key, &setting)?,
            "digest" => recap.digest = flag(&key, &setting)?,
            "digest_as_thread" => recap.digest_as_thread = flag(&key, &setting)?,
            _ => {
                return Err(unknown_key("recap", "recap", &key));
            }
        }
    }
    Ok(recap)
}
