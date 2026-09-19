use crate::*;

pub(crate) fn clear_remind(session_id: &str) {
    pns_application::clear_remind(
        &pns_adapters::FileRemindRecords::new(state_dir()),
        session_id,
        |warning| eprintln!("{warning}"),
    );
}

pub(crate) fn arm_remind(session_id: &str, event: &pns_domain::EventArgs) {
    let state = state_dir();
    pns_application::ArmRemind {
        records: &pns_adapters::FileRemindRecords::new(state.clone()),
        jobs: &pns_adapters::FileJobSpool::new(state),
    }
    .run(session_id, event, remind_delay_secs, now_secs, |warning| {
        eprintln!("{warning}")
    });
}

/// The schedule that means the reminder is off, in the composition root's own
/// spelling of `config`'s default.
pub(crate) const REMIND_OFF: u64 = 0;

/// How long an unanswered approval waits before it is carded again, or
/// `REMIND_OFF`.
///
/// AN UNREADABLE CONFIG MEANS OFF, which is `focus_silence`'s reading and for
/// the same reason: a file nobody can parse asked for nothing, and a feature
/// that INTERRUPTS must not be switched on by a parse failure. This
/// deliberately differs from `[recap]`, whose fallback is on because it
/// delivers something the operator is owed.
pub(crate) fn remind_delay_secs() -> u64 {
    let home = std::env::var("HOME").unwrap_or_default();
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config.remind_delay_secs,
        _ => REMIND_OFF,
    }
}
