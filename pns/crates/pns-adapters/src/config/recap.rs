use super::*;

/// `retain`'s range: an hour at the floor, so a value is long enough to hold
/// the window the recap it feeds is about, and a year at the ceiling, past
/// which the table is a log nobody reads rather than a recap source.
///
/// ZERO IS REFUSED BY NAME by `nonzero_duration_key`, exactly as `[remind]
/// delay` refuses it: a retention of nothing empties the store on the next
/// tick, which is a switch rather than a duration, and this key has no off.
fn retain_range() -> RangeInclusive<Duration> {
    Duration::from_secs(3600)..=Duration::from_secs(365 * 24 * 60 * 60)
}

/// `minimum_away`'s range: zero at the floor, which leaves the event count as
/// the only bar, and a day at the ceiling.
fn minimum_away_range() -> RangeInclusive<Duration> {
    Duration::ZERO..=Duration::from_secs(24 * 60 * 60)
}

/// `[recap]`'s switches, each starting at its default and moved only by a key
/// that states it.
///
/// NO KEY HERE DOUBLES AS ITS OWN SWITCH. `[recap.summarizer]` and every
/// `[recap.sources]` command are off by being UNSET, which is a state the key
/// already has, so none carries a magic value that means off and none needs an
/// `enabled` beside it; an empty value is refused by name instead, because it
/// names a thing pns would then try and fail to use.
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
            "minimum_away" => {
                recap.minimum_away = Duration::from_secs(duration_key(
                    "recap",
                    "minimum_away",
                    &setting,
                    minimum_away_range(),
                )?);
            }
            "sources" => recap.sources = parse_recap_sources(setting)?,
            "nightshift" => recap.periods.nightshift = period("nightshift", &setting)?,
            "morning" => recap.periods.morning = period("morning", &setting)?,
            "afternoon" => recap.periods.afternoon = period("afternoon", &setting)?,
            "evening" => recap.periods.evening = period("evening", &setting)?,
            "week_starts_on" => recap.week_starts_on = week_start(&setting)?,
            "rows_per_section" => recap.rows_per_section = rows_per_section(&setting)?,
            "review_notes_glob" => recap.review_notes_glob = Some(note_glob(&setting)?),
            "summarizer" => recap.summarizer = parse_recap_summarizer(setting)?,
            "pregenerate" => recap.pregenerate = pregenerate(&setting)?,
            "retain" => {
                recap.retain = Duration::from_secs(nonzero_duration_key(
                    "recap",
                    "retain",
                    &setting,
                    retain_range(),
                )?);
            }
            "replay_card" => recap.replay_card = flag(&key, &setting)?,
            "post_window_recap" => recap.post_window_recap = flag(&key, &setting)?,
            _ => {
                return Err(unknown_key("recap", "recap", &key));
            }
        }
    }
    // THE FOUR PERIODS ARE JUDGED TOGETHER, once, after every arm has run.
    // A gap or an overlap is a property of the SET, so checking it inside an
    // arm would refuse a config whose later key was about to close the gap.
    if let Some(fault) = pns_domain::recap::window::tiling_fault(&recap.periods) {
        return Err(ConfigError::Invalid(fault));
    }
    Ok(recap)
}

/// `pregenerate`, the windows whose summary the gateway writes in the
/// background at each window's end.
///
/// EVERY WORD IS A WINDOW NAME, checked here rather than at the tick: a typo
/// would otherwise be a window the gateway silently never pregenerates, which
/// reads as a summarizer that is not running.
///
/// AN EMPTY LIST IS THE SHIPPED DEFAULT and means the gateway writes none.
fn pregenerate(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let named = strings("recap", "pregenerate", "a list of window names", setting)?;
    for window in &named {
        if pns_domain::recap::window::parse_window(window).is_none() {
            return Err(ConfigError::Invalid(format!(
                "`recap` key `pregenerate` names `{window}`, which is no window; it takes {}",
                pns_domain::recap::window::WINDOW_WORDS.join(", ")
            )));
        }
    }
    Ok(named)
}
