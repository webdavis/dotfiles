use super::*;

/// The `[plugins.mobile]` settings, but ONLY when the table is ARMED: switched
/// on, and naming a backend this binary answers.
///
/// THE ONE READER OF THAT TABLE. The push token, the watching-card toggle and
/// the submission deadline all come through here, so a table naming a backend
/// nothing implements contributes no settings ANYWHERE, rather than being
/// refused on the paths that remembered to ask and honoured on the ones that
/// did not. A `submit_deadline_secs` written under `type = "pushover"` is a
/// number for a backend this binary has never heard of, and reading it as
/// moshi's is exactly the misattribution `type` exists to stop.
///
/// `Ok(None)` IS THE INERT TABLE: absent, or present with the switch off. That
/// is the reading `enabled_hue_table` and `home::enabled_router_table` already
/// give their own, and it covers the table's SETTINGS too, the deadline
/// included: one switch, one answer, rather than a per-key exception nobody
/// could predict from the flag they set. Nothing complains about it either,
/// because a line about a channel the operator turned off, on every event, is
/// noise. `pns doctor` is where a switched-off table naming no backend is
/// still made visible.
///
/// `Err` CARRIES THE REASON, never a bare `None`. A caller that collapsed the
/// two reported a missing token for a fault that was the type, and sent an
/// operator whose token was already correct to go and check it.
pub fn armed_mobile(config: &Config) -> Result<Option<&toml::Table>, String> {
    let Some(mobile) = config.plugins.get("mobile").filter(|mobile| mobile.enabled) else {
        return Ok(None);
    };
    mobile_backend(&mobile.settings)?;
    Ok(Some(&mobile.settings))
}

/// How long pns waits for moshi to acknowledge a submission, from
/// `[plugins.mobile] submit_deadline_secs`, with the default when no key states
/// one.
///
/// IT IS READ OFF THE ARMED MOBILE TABLE and nowhere else, through
/// `armed_mobile`. Plugin settings reach this layer free-form, so every
/// plugin's table would answer a key spelled this way, and a reader that asked
/// the wrong one would take a number the operator wrote for something else.
/// The backend is part of that same question: a deadline under a table naming
/// no compiled-in backend is refused rather than read as moshi's.
///
/// THE REFUSALS ARE LOUD AND NAMED, because the caller falls back to the
/// default and a silent fallback is the operator asking for something, not
/// getting it, and being told nothing.
pub fn submit_deadline(config: &Config) -> Result<Duration, ConfigError> {
    let Some(mobile) = armed_mobile(config).map_err(ConfigError::Invalid)? else {
        return Ok(Duration::from_secs(DEFAULT_SUBMIT_DEADLINE_SECS));
    };
    let Some(stated) = mobile.get("submit_deadline_secs") else {
        return Ok(Duration::from_secs(DEFAULT_SUBMIT_DEADLINE_SECS));
    };
    let Some(count) = stated
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`mobile` key `submit_deadline_secs` has type `{}`, not a count of seconds",
            stated.type_str()
        )));
    };
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`mobile` key `submit_deadline_secs` is 0, which is the bound switched off by \
             accident: a deadline that expires before the daemon can answer costs the phone \
             card on every approval"
                .to_string(),
        ));
    }
    if count > MAX_SUBMIT_DEADLINE_SECS {
        return Err(ConfigError::Invalid(format!(
            "`mobile` key `submit_deadline_secs` is {count}, past the \
             {MAX_SUBMIT_DEADLINE_SECS}-second ceiling"
        )));
    }
    Ok(Duration::from_secs(count))
}

/// How long pns waits for moshi to acknowledge a submission before returning
/// no opinion.
///
/// FIVE SECONDS, and it is the crate's own house number for a local pipe that
/// should have been instant: `payload_deadline` bounds the same kind of thing
/// on the same hook with the same figure. THE WAIT IS A REGISTRATION, NOT A
/// HUMAN WAIT (measured 2026-08-29): `moshi-hook` writes one line to its
/// daemon's socket and returns as soon as the daemon answers, roughly a tenth
/// of a second, and the operator's own decision arrives later and by another
/// road. So five seconds is about thirty times the observed round trip, and a
/// wait past it is a daemon that stopped answering rather than an operator
/// taking their time.
pub const DEFAULT_SUBMIT_DEADLINE_SECS: u64 = 5;

/// The most that wait may be given. ONE HOUR, mirroring the summarizer's
/// ceiling rather than the harness's own PermissionRequest limit: another
/// tool's number is not ours to hard-code, and Codex's differs. There is no
/// off switch, because an unbounded wait is the defect and "off" would be a
/// key whose only function is to restore it.
pub(super) const MAX_SUBMIT_DEADLINE_SECS: u64 = 3600;
