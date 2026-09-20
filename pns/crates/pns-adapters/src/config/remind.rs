use super::*;

/// The delay that means the reminder is off. See `Config::remind_delay_secs`.
pub(super) const REMIND_OFF: u64 = 0;

/// `[remind]`'s one key, in `parse_daemon`'s shape: an unknown key inside the
/// table and a value of the wrong shape are each refused BY NAME rather than
/// half-read into a schedule the operator believes they set.
pub(super) fn parse_remind(value: toml::Value) -> Result<u64, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`remind` is not a table".to_string()));
    };
    let mut delay = REMIND_OFF;
    for (key, setting) in table {
        admits_flat("remind", &key)?;
        match key.as_str() {
            "delay" => delay = duration_key("remind", "delay", &setting, remind_delay_range())?,
            _ => {
                return Err(unknown_key("remind", "remind", &key));
            }
        }
    }
    Ok(delay)
}

/// `delay`'s range, WITH ZERO CARVED OUT by `parse_remind` above.
///
/// ZERO IS NOT A SCHEDULE AND IS NOT AN ERROR: it is the same statement as
/// writing no table, which is what makes this key the switch as well as the
/// timing. Every other value under the floor IS an error, because it is a
/// schedule the operator meant and pns will not run.
///
/// THE BOUNDS THEMSELVES ARE THE POLICY CRATE'S, so the file, the flag and the
/// JSON field are held to one range. The ceiling is an hour, the same number
/// `MAX_SUMMARIZER_DEADLINE_SECS` carries.
pub fn remind_delay_range() -> RangeInclusive<Duration> {
    pns_domain::remind::DELAY_RANGE
}

/// The one refusal that reads TWO tables, and the reason it cannot live in
/// either of them: `[lights.blocked] give_up_after_secs` and `[remind] delay`
/// are each perfectly good numbers on their own and contradict each other only
/// together, so the check belongs where the whole file is in hand.
///
/// THE BACKSTOP DARKENS AN UNANSWERED WAIT'S LAMP at `give_up_after_secs` and
/// the reminder CARDS the same wait at `delay`. Written the shorter way round,
/// the engine gives up on a wait before it has ever nudged about it, so the
/// nudge's own lamp could never be lit and the backstop is bounding an
/// abandonment the operator has not yet been told about. That is a config that
/// cannot do what it says, refused at load in `ends_agree`'s style rather than
/// worked around at runtime by a mechanism that would have to tell a live
/// session from a crashed one.
///
/// EQUAL IS ACCEPTED. Shorter is the contradiction; reaching the bound exactly
/// as the nudge fires is a tight config the operator may well mean.
///
/// A REMINDER THAT IS OFF CONTRADICTS NOTHING, and neither does a file with no
/// `[lights]` table: with no nudge or no lamp there is no pair to disagree.
///
/// NEITHER OF THOSE TWO GUARDS IS OBSERVABLE TODAY, said here because a
/// mutation of either survives the suite and the next reader deserves to know
/// it is dead code rather than an untested branch. `REMIND_OFF` is zero and
/// `give_up_after_secs` has a floor of 60, so the comparison below is already
/// false for an off reminder; and `DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS` (16
/// hours) sits far above `MAX_DELAY_SECS` (one hour), so a config with
/// no `[lights]` table could not trip the check even if it were read at its
/// default. They stay because each states its own case out loud, and because
/// what makes them dead is a coupling between two bounds that have nothing
/// else to do with each other: either one moving would make a guard live
/// again with nothing at the seam to say so.
pub(super) fn backstop_outlasts_the_reminder(config: &Config) -> Result<(), ConfigError> {
    let Some(lights) = config.lights.as_ref() else {
        return Ok(());
    };
    if config.remind_delay_secs == REMIND_OFF {
        return Ok(());
    }
    let give_up = lights.blocked.give_up_after_secs;
    if give_up < config.remind_delay_secs {
        return Err(ConfigError::Invalid(format!(
            "`lights.blocked` key `give_up_after_secs` is {give_up}, below `remind` key \
             `delay` {}, so the lamp would be given up on before the nudge it \
             belongs to has ever fired",
            config.remind_delay_secs
        )));
    }
    Ok(())
}
