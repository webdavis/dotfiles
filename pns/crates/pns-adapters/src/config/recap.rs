use super::*;

/// The most any summarizer may be given. ONE HOUR, which is fifteen times the
/// default, so no honest backend on any machine meets it; see
/// `summarizer_deadline_range` for the two failures that live past it.
pub(super) const MAX_SUMMARIZER_DEADLINE_SECS: u64 = 3600;

/// `summarizer_deadline`, BOUNDED ON BOTH SIDES with zero carved out by
/// `duration_value`: a deadline of nothing simply cannot be met, so the recap
/// falls to the plain lists and says it did.
///
/// THE FLOOR IS ONE MILLISECOND, so a test can prove expiry without waiting on
/// a real backend.
///
/// THE TOP END IS REFUSED BY NAME, and two things break past it, neither
/// visible where it happens. NOTHING SUPERVISES THE DETACHED RECAP CHILD,
/// which `spawn_recap` states outright: at four minutes that is fine, and at a
/// day it is one child plus one wedged backend held for a day, with a second
/// pair arriving at the next return moment. AND A DURATION PAST THE CEILING
/// PANICS at `Instant::now() + deadline` (MEASURED: "overflow when adding
/// duration to instant") inside a process whose stderr is /dev/null and whose
/// exit code nobody reads, so the recap simply vanishes after the card has
/// said it is coming. A refusal the operator reads beats a silence they
/// cannot.
fn summarizer_deadline_range() -> RangeInclusive<Duration> {
    Duration::from_millis(1)..=Duration::from_secs(MAX_SUMMARIZER_DEADLINE_SECS)
}

/// `[recap]`'s switches, each starting at its default and moved only by a key
/// that states it.
///
/// NO KEY HERE DOUBLES AS ITS OWN SWITCH. `summarizer` and `repositories`
/// are off by being UNSET, which is a state the key already has, so neither
/// carries a magic value that means off and neither needs an `enabled` beside
/// it; an empty value is refused by name instead, because it names a thing pns
/// would then try and fail to use.
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
            "minimum_events" => recap.minimum_events = threshold(&setting)?,
            "repositories" => recap.repositories = repositories(&setting)?,
            "review_notes_glob" => recap.review_notes_glob = Some(note_glob(&setting)?),
            "summarizer" => recap.summarizer = Some(argv(&setting)?),
            "summarizer_deadline" => {
                recap.summarizer_deadline = duration_value(
                    "recap",
                    "summarizer_deadline",
                    &setting,
                    summarizer_deadline_range(),
                )?;
            }
            "replay_card" => recap.replay_card = flag(&key, &setting)?,
            "post_window_recap" => recap.post_window_recap = flag(&key, &setting)?,
            _ => {
                return Err(unknown_key("recap", "recap", &key));
            }
        }
    }
    Ok(recap)
}
