use crate::legacy::{Remind, remind_switch};
use crate::*;

pub(crate) fn clear_remind(session_id: &str) {
    pns_application::clear_remind(
        &pns_adapters::FileRemindRecords::new(state_dir()),
        session_id,
        |warning| eprintln!("{warning}"),
    );
}

pub(crate) fn arm_remind(session_id: &str, event: &pns_domain::EventArgs, after_secs: u64) {
    let state = state_dir();
    pns_application::ArmRemind {
        records: &pns_adapters::FileRemindRecords::new(state.clone()),
        jobs: &pns_adapters::FileJobSpool::new(state),
    }
    .run(session_id, event, after_secs, now_secs, |warning| {
        eprintln!("{warning}")
    });
}

/// The schedule that means the reminder is off, in the composition root's own
/// spelling of `config`'s default.
pub(crate) const REMIND_OFF: u64 = 0;

/// How long THIS call's unanswered approval waits before it is carded again,
/// or `REMIND_OFF`.
///
/// MOST SPECIFIC FIRST: the switch this call carried, then the producer's own
/// `[producer.<name>] remind`, then off. The producer's NAME decides nothing
/// on its own, which is the whole point: only a harness knows whether it sends
/// the answered signal a reminder needs, so the harness says so per call and
/// the table is there for a producer nobody can pass a flag to.
///
/// `--remind` WITH NO DELAY ANYWHERE IS A REFUSAL rather than a guess, and it
/// names both ways out. A producer's config entry with no delay is the feature
/// off instead, because an unset `[remind] delay` is already how the file says
/// "no reminder".
pub(crate) fn remind_delay(argv: &[String], producer: &str) -> Result<u64, String> {
    match remind_switch(argv)? {
        Some(Remind::Off) => Ok(REMIND_OFF),
        Some(Remind::After(seconds)) => match backstop_secs() {
            Some(give_up) if give_up < seconds => Err(remind_outlasts_backstop(seconds, give_up)),
            _ => Ok(seconds),
        },
        Some(Remind::Configured) => match remind_delay_secs() {
            REMIND_OFF => Err(NO_DELAY_TO_REMIND_AT.to_string()),
            delay => Ok(delay),
        },
        None => Ok(loaded_config().map_or(REMIND_OFF, |config| {
            match config.producer_remind.get(producer) {
                Some(true) => config.remind_delay_secs,
                _ => REMIND_OFF,
            }
        })),
    }
}

/// What `--remind` with no delay to run at is told, naming both fixes.
const NO_DELAY_TO_REMIND_AT: &str = "--remind has no delay to run at; \
set `delay` in the `[remind]` table of ~/.config/pns/config.toml, \
or pass --remind=<duration>";

/// `[lights.blocked] give_up_after_secs`, the loader's own reading, so
/// `--remind=<duration>` is held to the invariant the loader already enforces
/// for `[remind] delay`: `backstop_outlasts_the_reminder` in
/// `pns-adapters/src/config/remind.rs`.
fn backstop_secs() -> Option<u64> {
    loaded_config()?
        .lights
        .as_ref()
        .map(|lights| lights.blocked.give_up_after_secs)
}

/// What `--remind=<duration>` naming a wait past the lamp's own backstop is
/// told, in the loader's wording for the same contradiction.
fn remind_outlasts_backstop(seconds: u64, give_up: u64) -> String {
    format!(
        "--remind is {seconds}s, above `lights.blocked` key `give_up_after_secs` \
         ({give_up}s), so the lamp would be given up on before the nudge it \
         belongs to has ever fired"
    )
}

/// How long an unanswered approval waits, off `[remind] delay` alone.
///
/// AN UNREADABLE CONFIG MEANS OFF, which is `focus_silence`'s reading and for
/// the same reason: a file nobody can parse asked for nothing, and a feature
/// that INTERRUPTS must not be switched on by a parse failure. This
/// deliberately differs from `[recap]`, whose fallback is on because it
/// delivers something the operator is owed.
pub(crate) fn remind_delay_secs() -> u64 {
    loaded_config().map_or(REMIND_OFF, |config| config.remind_delay_secs)
}

/// The parsed config, or None when there is none to read. One loader, so the
/// delay and the producer's entry are read off ONE parse of the file.
fn loaded_config() -> Option<Box<pns_adapters::Config>> {
    let home = std::env::var("HOME").unwrap_or_default();
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => Some(config),
        _ => None,
    }
}
