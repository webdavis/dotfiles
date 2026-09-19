use crate::*;
use pns_application::RemindRecords;

// --- the reminder ----------------------------------------------------------------

/// `pns remind`: one card about every approval nobody has answered, or silence.
///
/// RUN BY THE DAEMON AND TYPEABLE BY THE OPERATOR, which is what makes the
/// drill forceable without waiting out a timer. It PRINTS what it did, one
/// line, in `recap`'s shape.
///
/// OWNERSHIP IS TAKEN AT TWO LEVELS, and they answer two different questions.
/// The WINDOW is claimed once, before anything is enumerated (`claim_fire`), so
/// two processes woken by two jobs in one tick produce one card between them
/// rather than one card each. Each RECORD is then claimed by rename before it
/// is read for anything, which is what stops a single approval being counted
/// twice by a fire that broke in after a stale window claim aged out. The window uses exclusive creation; each record uses rename. The
/// measurements are in
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`.
///
/// THE ORDER IS THE SAFE ONE AT EVERY STEP. The markers are written BEFORE the
/// card and the claims removed AFTER it: a crash before the card leaves
/// approvals marked and silent, a crash after it leaves claims nothing
/// re-enumerates, and neither ordering can produce a SECOND card, which is the
/// property that matters.
pub(crate) fn remind_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, per the house rule that an unknown argument
    // never falls through to help with exit 0. `pns remind <session>` is a command
    // an operator would believe narrowed the fire, and coalescing means nothing
    // here can honour it.
    if !crate::arguments_after_subcommand().is_empty() {
        eprintln!("{REMIND_USAGE}");
        return 2;
    }
    let state = state_dir();
    let records = pns_adapters::FileRemindRecords::new(state);
    // A CONFIG THAT TURNED THE FEATURE OFF BETWEEN ARMING AND FIRING MEANS NO
    // NUDGE, and the records go with it: the operator cancelled the timer, and
    // a card from it would be the feature ignoring them.
    let after_secs = remind_delay_secs();
    if after_secs == REMIND_OFF {
        let dropped = records.clear_pending();
        println!("pns remind: the reminder is off; {dropped} waiting approval(s) dropped");
        return 0;
    }
    // NO CLOCK IS NO NUDGE. Every input this cannot read resolves to silence,
    // and a wait nothing can measure is one of them.
    let Some(now) = now_secs() else {
        eprintln!("pns remind: this machine has no clock to measure a wait against");
        return 0;
    };
    match (pns_application::RunRemind {
        records: &records,
        notifier: &RemindNotification,
    })
    .run(now, after_secs, |warning| eprintln!("{warning}"))
    {
        pns_application::RemindOutcome::Busy => {}
        pns_application::RemindOutcome::Nothing => println!("pns remind: nothing is waiting"),
        pns_application::RemindOutcome::Nudged(count) => {
            // ATTEMPTED, NEVER SENT. A mute, Focus or an empty plan may suppress
            // delivery, so the nudge cannot truthfully report a sent card.
            println!("pns remind: {count} waiting; one card attempted");
        }
    }
    0
}

struct RemindNotification;
impl pns_application::RaiseNotification for RemindNotification {
    fn raise(&self, event: &pns_domain::EventArgs) {
        // PNS_SKIP_PHONE belongs to the earlier blocking process and is not
        // copied into this later fire. The one coalesced card has no session
        // payload, and Attempt::Nudge returns before touching a needs marker.
        run_event(
            event,
            &system_probes(),
            &HookPayload::default(),
            Attempt::Nudge,
        );
    }
}
pub(crate) const REMIND_USAGE: &str = "pns: usage: pns remind (it takes no arguments: one fire cards every \
outstanding approval at once)";

/// What `pns nag` answers now: the word that replaced it, and no fire.
pub(crate) fn retired_nag() -> i32 {
    eprintln!("pns: the nag is the reminder now: run `pns remind`");
    eprintln!("{REMIND_USAGE}");
    2
}
