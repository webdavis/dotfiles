use crate::*;

/// `pns stale`: one page about every session that has been blocked past the
/// window, or silence.
///
/// RUN BY THE DAEMON AND TYPEABLE BY THE OPERATOR, which is what makes the
/// escalation forceable without waiting out an hour: `pns stale` typed at the
/// desk fires the same path the job does.
///
/// WHAT IT SAYS AND WHERE. The daemon points a job's stdout at `/dev/null` and
/// INHERITS its stderr into `~/.local/log`, so the ordinary line goes to
/// stdout for the operator who typed it and everything that must reach the
/// unattended log goes to stderr: no clock, a suppressed fire, and a page the
/// gateway would not take.
pub(crate) fn stale_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, per the house rule that an unknown argument
    // never falls through to help with exit 0. `pns stale <session>` is a
    // command an operator would believe narrowed the fire, and one fire covers
    // every waiting session at once.
    if !crate::arguments_after_subcommand().is_empty() {
        eprintln!("{STALE_USAGE}");
        return 2;
    }
    // ONE GUARD, AND IT IS THE FIRE'S. The window is read here and judged
    // there, so the `Off` arm below is the only place that answers a feature
    // switched off between arming and firing; a second check here would be the
    // same policy stated twice, with the arm unreachable.
    //
    // WHICH IS WHY THE CLOCK IS READ FIRST. A machine with no clock cannot ask
    // the fire anything, off or not, so it says so and stops.
    let settings = stale_settings();
    // NO CLOCK IS NO PAGE. Every input this cannot read resolves to silence,
    // and a wait nothing can measure is one of them.
    let probes = system_probes();
    let Some(now) = probes.now_secs() else {
        eprintln!("pns stale: this machine has no clock to measure a block against");
        return 0;
    };
    // ONE READING FOR THE WHOLE FIRE, off the same probe set every event path
    // uses: whether the operator can act and how long the desk has been idle
    // are two questions about one moment.
    let reading =
        pns_application::operator_surface_reading(&probes, &overrides_from_env(), Some(now));
    match (pns_application::EscalateStaleBlocks {
        waits: &pns_adapters::SqliteStore::new(state_dir()),
        notifier: &StaleNotification {
            route: settings.route.clone(),
        },
    })
    .run(now, settings.window, &settings.route, &reading)
    {
        pns_application::StaleOutcome::Off => {
            println!("pns stale: the stale-block escalation is off")
        }
        pns_application::StaleOutcome::Nothing => println!("pns stale: nothing is stuck"),
        pns_application::StaleOutcome::Held { waiting, why } => {
            // ON STDERR, because a suppressed page is the one thing about this
            // fire an unattended log has to carry: the block is still stuck.
            eprintln!(
                "pns stale: {waiting} stuck session(s) held back ({})",
                why.said()
            );
        }
        pns_application::StaleOutcome::Paged(count) => {
            // ATTEMPTED, NEVER SENT, which is the reminder's own honesty: the row is
            // stamped before the page, so a mute or a Focus can suppress the
            // delivery of a page this run will not make twice.
            println!("pns stale: {count} stuck session(s); one page attempted each");
        }
    }
    0
}

/// The page itself, through the ordinary event path.
///
/// IT CARRIES THE ROUTE'S NAME TO SAY IT. The page is already addressed to
/// this route by the fire that built it; this copy is here so the unattended
/// line can name where a page that was not confirmed was headed.
struct StaleNotification {
    route: String,
}
impl pns_application::RaiseNotification for StaleNotification {
    fn raise(&self, event: &pns_domain::EventArgs) {
        // `Attempt::Nudge` FOR THE REMINDER'S REASON: this is a second card about
        // an event already recorded, so it must not journal a miss, count as
        // activity, claim the return moment or pulse a lamp again.
        let landed = run_event(
            event,
            &system_probes(),
            &HookPayload::default(),
            Attempt::Nudge,
        );
        if landed == event_flow::Landed::No {
            // SAID RATHER THAN SWALLOWED, on the stream the daemon keeps. The
            // page's route is the one thing about it that is not
            // the ordinary event path's problem: a gateway that refuses it
            // answers 401 or 404, the ledger records the refusal for
            // `pns failures`, and this line is what puts it in front of an
            // operator who is not watching.
            //
            // "NOT CONFIRMED" RATHER THAN "DID NOT ARRIVE", because those are
            // different facts and only one of them is known here. On a machine
            // running EXECUTABLE channels a channel that RAN answers `Silent`
            // whatever the gateway then said (the limit `post_return_recap`
            // states), so a page that landed reads as unconfirmed there; a
            // line claiming it never arrived would be the false half.
            eprintln!(
                "pns stale: the page about {} is not confirmed on the {} route",
                event.project, self.route
            );
        }
    }
}

pub(crate) const STALE_USAGE: &str = "pns: usage: pns stale (it takes no arguments: one fire pages \
about every session stuck past the window)";
