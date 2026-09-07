use super::*;

/// The schedule that means the nag is off. See `Config::nag_after_secs`.
pub(super) const NAG_OFF: u64 = 0;

/// `[nag]`'s one key, in `parse_daemon`'s shape: an unknown key inside the
/// table and a value of the wrong shape are each refused BY NAME rather than
/// half-read into a schedule the operator believes they set.
pub(super) fn parse_nag(value: toml::Value) -> Result<u64, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`nag` is not a table".to_string()));
    };
    let mut after_secs = NAG_OFF;
    for (key, setting) in table {
        admits_flat("nag", &key)?;
        match key.as_str() {
            "after_secs" => after_secs = nag_schedule(&setting)?,
            _ => {
                return Err(unknown_key("nag", "nag", &key));
            }
        }
    }
    Ok(after_secs)
}

/// `after_secs`, in whole seconds, BOUNDED ON BOTH SIDES with zero carved out.
///
/// ZERO IS NOT A SCHEDULE AND IS NOT AN ERROR: it is the same statement as
/// writing no table, which is what makes this key the switch as well as the
/// timing. Every other value under the floor IS an error, because it is a
/// schedule the operator meant and pns will not run.
///
/// THE FLOOR IS THIRTY SECONDS. A nudge arriving before the operator could
/// plausibly have picked up their phone is the stacking this design forbids,
/// and thirty is low enough that the feature can be drilled in half a minute.
///
/// THE CEILING IS AN HOUR, mirroring `MAX_SUMMARIZER_DEADLINE_SECS` rather than
/// any harness number. It must also sit inside the daemon's own registration
/// window (`daemon::DUE_WINDOW_SECS`, thirty days), which it does with room to
/// spare, and it is what keeps `2 * after_secs` in the staleness cap far from
/// any arithmetic edge.
///
/// REFUSED RATHER THAN CLAMPED, in `min_events`'s style: a silently corrected
/// schedule is a schedule the operator believes they set.
pub(super) fn nag_schedule(setting: &toml::Value) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`nag` key `after_secs` has type `{}`, not a count of seconds",
            setting.type_str()
        )));
    };
    if count == NAG_OFF {
        return Ok(NAG_OFF);
    }
    if !(MIN_NAG_AFTER_SECS..=MAX_NAG_AFTER_SECS).contains(&count) {
        return Err(ConfigError::Invalid(format!(
            "`nag` key `after_secs` is {count}, outside the {MIN_NAG_AFTER_SECS} to \
             {MAX_NAG_AFTER_SECS} second range; 0 is the feature off"
        )));
    }
    Ok(count)
}

/// The shortest nag anyone may schedule. See `nag_schedule`.
pub(super) const MIN_NAG_AFTER_SECS: u64 = 30;

/// The longest. See `nag_schedule`.
pub(super) const MAX_NAG_AFTER_SECS: u64 = MAX_SUMMARIZER_DEADLINE_SECS;

/// The one refusal that reads TWO tables, and the reason it cannot live in
/// either of them: `[lights.blocked] give_up_after_secs` and `[nag] after_secs`
/// are each perfectly good numbers on their own and contradict each other only
/// together, so the check belongs where the whole file is in hand.
///
/// THE BACKSTOP DARKENS AN UNANSWERED WAIT'S LAMP at `give_up_after_secs` and
/// the nag CARDS the same wait at `after_secs`. Written the shorter way round,
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
/// A NAG THAT IS OFF CONTRADICTS NOTHING, and neither does a file with no
/// `[lights]` table: with no nudge or no lamp there is no pair to disagree.
///
/// NEITHER OF THOSE TWO GUARDS IS OBSERVABLE TODAY, said here because a
/// mutation of either survives the suite and the next reader deserves to know
/// it is dead code rather than an untested branch. `NAG_OFF` is zero and
/// `give_up_after_secs` has a floor of 60, so the comparison below is already
/// false for an off nag; and `DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS` (16 hours)
/// sits far above `MAX_NAG_AFTER_SECS` (one hour), so a config with no
/// `[lights]` table could not trip the check even if it were read at its
/// default. They stay because each states its own case out loud, and because
/// what makes them dead is a coupling between two bounds that have nothing
/// else to do with each other: either one moving would make a guard live
/// again with nothing at the seam to say so.
pub(super) fn backstop_outlasts_the_nag(config: &Config) -> Result<(), ConfigError> {
    let Some(lights) = config.lights.as_ref() else {
        return Ok(());
    };
    if config.nag_after_secs == NAG_OFF {
        return Ok(());
    }
    let give_up = lights.blocked.give_up_after_secs;
    if give_up < config.nag_after_secs {
        return Err(ConfigError::Invalid(format!(
            "`lights.blocked` key `give_up_after_secs` is {give_up}, below `nag` key \
             `after_secs` {}, so the lamp would be given up on before the nudge it \
             belongs to has ever fired",
            config.nag_after_secs
        )));
    }
    Ok(())
}
