//! Raising the notification that says a page did not arrive.
//!
//! THE READ IS THE SOURCE, not the delivery's return value. A leg's id, its
//! retry count and whether pns has given up are all facts the ledger settles
//! when it records the attempt, and reading them back is what keeps this
//! module's notification and `pns failures` saying the same thing about the same
//! failure. Passing them forward from the attempt would be a second copy of a
//! record that already exists, free to disagree with it.
//!
//! IT IS NOT ON THE LEDGER'S SIDE. A failure notice that failed is not itself
//! recorded and not retried: the log, `pns failures` and `pns doctor` all still
//! hold the failure, and a queue of notices about a broken queue is the loop
//! this stays out of.
//!
//! NEVER THROUGH THE DESTINATION THAT FAILED. A 404 on a hermes route means the
//! message is not in Discord, and a moshi refusal means the phone is not
//! reachable, so a notice sent that way arrives nowhere and tells the operator
//! nothing. The banner is exempt by construction: it is local, so it can never
//! be the surface a delivery failure just took away.

use pns_adapters::SqliteStore;
use pns_application::StoredFailure;
use pns_domain::failure::{self, Failure, NotificationSurface};

/// How far back one pass looks. A pass records at most a handful of failures,
/// and the `failed_at` filter below is what actually bounds the set; this is
/// only the window the ledger is asked for.
const SCAN: u32 = 20;

/// What the phone gets, when the phone gets anything.
///
/// AN `Option` AT THE CALL SITE ANSWERS THE PRESENCE QUESTION ONCE. At the desk
/// the banner is already in front of the operator and a card is the same news
/// on a second screen, so there is nothing to carry and this is `None`.
struct PhoneCard {
    /// `[plugins.mobile] token`, or `None` for a table that is off or unset.
    /// Carried rather than checked, because the channel's own refusal names the
    /// config key and this module has no better sentence than that one.
    token: Option<String>,
    /// `[failures] serve`, which decides which pointer the card's fix line
    /// carries: the local page, or the place the full form actually is.
    serve: bool,
}

/// Announce every failure this pass recorded that warrants it.
///
/// `since` is the moment the pass began, and it is what makes this idempotent
/// across passes: a leg that failed in an earlier pass has an older
/// `failed_at`, so a second call never re-announces it.
pub(crate) fn announce(store: &SqliteStore, since: u64) {
    let Ok(failures) = store.failing_legs(SCAN) else {
        // NOTHING IS SAID ABOUT AN UNREADABLE LEDGER HERE. That is the delivery
        // health alarm's own subject, it already reports it, and a second voice
        // saying it would be two banners for one fault.
        return;
    };
    let speaking_for = to_announce(&failures, since);
    // NOTHING IS READ FOR A PASS WITH NOTHING TO SAY. The config load and the
    // presence probe below both cost real work, and the overwhelmingly common
    // pass is one where a delivery failed and this filter clears it.
    if speaking_for.is_empty() {
        return;
    }
    let pns = crate::command_click::pns_path();
    let phone = phone_card();
    for stored in speaking_for {
        raise(&crate::command_failures::compose(stored), &pns, &phone);
    }
}

/// Which of the ledger's failing legs this pass speaks for.
///
/// SPLIT OUT SO IT CAN BE PROVEN, because the half above it is a notifier spawn
/// and the half below is the whole of the decision. A test-only copy of this
/// filter would be free to disagree with the one that runs.
fn to_announce(failures: &[StoredFailure], since: u64) -> Vec<&StoredFailure> {
    failures
        .iter()
        .filter(|stored| stored.failed_at >= since)
        .filter(|stored| failure::warrants_notification(stored.retries, stored.deadlettered))
        .collect()
}

/// The card's settings, or `None` when the operator is at the desk.
///
/// THE SAME PRESENCE RULE EVERY OTHER CARD FOLLOWS, read the way
/// `forward_to_moshi` reads it: anything but `Desk`. A failure is not routine
/// enough to consult the watch-card toggle, which exists so a long command does
/// not card someone who is watching it finish; the question here is only whether
/// the operator is somewhere the banner cannot reach.
fn phone_card() -> Option<PhoneCard> {
    let probes = crate::system_probes();
    if pns_application::operator_surface(&probes, &crate::overrides_from_env(), probes.now_secs())
        == pns_domain::surface::Surface::Desk
    {
        return None;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(pns_adapters::LoadOutcome::Loaded(config)) =
        pns_adapters::load_config(&pns_adapters::config_path(&home))
    else {
        return None;
    };
    Some(PhoneCard {
        // NOT `read_mobile`, WHICH COMPLAINS. It prints the config error for a
        // table naming no compiled-in backend, and the event path this runs
        // inside has already printed that same line for that same event: one
        // fault, one complaint. The token is all this needs.
        token: pns_adapters::armed_mobile(&config)
            .ok()
            .flatten()
            .and_then(pns_adapters::moshi_secret),
        serve: config.failures.serve,
    })
}

/// One notification for one failure: a banner always, and a card when the
/// operator is not at the desk.
fn raise(failure: &Failure, pns_path: &str, phone: &Option<PhoneCard>) {
    let args = pns_adapters::notifier_args(
        &failure.title(),
        &failure::notification(failure, NotificationSurface::Banner),
        Some("default"),
        pns_adapters::DEFAULT_TERMINAL_BUNDLE_ID,
        &failure::click_command(pns_path, failure.id),
    );
    // The outcome is DROPPED. A banner that will not post is the delivery
    // health alarm's subject rather than this module's, and there is no second
    // local surface to report it on.
    let _ = pns_application::CommandRunner::run(
        &pns_adapters::SystemCommandRunner,
        "terminal-notifier",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    if let Some(phone) = phone {
        push(failure, phone);
    }
}

/// The phone's surface for this failure, or `None` when the phone is what
/// failed.
///
/// THE MOBILE LEG IS THE ONE THAT SILENCES ITS OWN CARD. Pushing a card about a
/// push that was refused sends it through the destination that just refused
/// one, so it arrives nowhere and the operator learns nothing.
fn card_surface(failure: &Failure, serve: bool) -> Option<NotificationSurface> {
    if failure.destination == failure::DESTINATION_MOBILE {
        return None;
    }
    Some(NotificationSurface::Phone {
        serve,
        // WHETHER THE FULL FORM IS IN DISCORD, which is the only thing the fix
        // line's third choice turns on: hermes carries it, so a hermes failure
        // means there is nothing there to point at.
        hermes_failed: failure.destination == failure::DESTINATION_HERMES,
    })
}

fn push(failure: &Failure, phone: &PhoneCard) {
    let Some(surface) = card_surface(failure, phone.serve) else {
        return;
    };
    let body = failure::notification(failure, surface);
    let event = pns_domain::Event {
        agent: failure.agent.clone(),
        state: "failed".to_string(),
        title: failure.title(),
        preview: body.clone(),
        message: body,
        ..Default::default()
    };
    // NO PANE, so the card carries no herdr deep link. The failure did not
    // happen in a pane, and a link to whichever pane the daemon happens to be
    // running in would open somewhere the operator was not working.
    let _ = pns_application::NotificationDestination::deliver(
        &crate::channel_dispatch::moshi_channel(phone.token.clone()),
        &pns_application::DeliveryRequest {
            producer: "pns",
            request_id: None,
            event: &event,
            route: "",
            mode: pns_domain::routing::ReportMode::Silent,
        },
    );
}

#[cfg(test)]
#[path = "failure_notice/tests.rs"]
mod tests;
