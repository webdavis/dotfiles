use crate::{Clock, DecisionRing, Journal};
use pns_domain::{Delivery, EventArgs, PresenceDecision, PresenceStatus};
use pns_domain::{
    doctor::{Check, LightsReport, Outcome, PairingReport},
    routing::Leg,
};
mod imports;
mod sections;
pub use imports::ImportFailure;

pub struct RunDoctor<'a, R, C> {
    pub checks: &'a [Check],
    pub records: &'a R,
    pub clock: &'a C,
    pub replay_card: bool,
    pub nag_after_secs: u64,
    /// How much of each recorded decision to print. A FIELD RATHER THAN AN
    /// ARGUMENT because it is the operator's standing answer for the whole
    /// report, not one section's parameter.
    pub decisions: pns_domain::doctor::Detail,
}

pub struct DoctorActions<D, P, PR, PA, F, DA, L, I, H, RO> {
    pub deliver: D,
    pub pulse: P,
    pub presence: PR,
    pub pairing: PA,
    pub focus: F,
    pub daemon: DA,
    pub lamps: L,
    pub imports: I,
    pub delivery_health: H,
    /// Whether the gateway serves each route pns has posted to. A closure
    /// rather than a value, so the doctor pays for the probes only when it
    /// reaches this section.
    pub routes: RO,
}

impl<R: DecisionRing + Journal, C: Clock> RunDoctor<'_, R, C> {
    pub fn run<D, P, PR, PA, F, DA, L, I, H, RO>(
        &self,
        mut actions: DoctorActions<D, P, PR, PA, F, DA, L, I, H, RO>,
        mut emit: impl FnMut(pns_domain::doctor::Item),
    ) -> i32
    where
        D: FnOnce(&[Leg], &EventArgs) -> Vec<(Leg, Delivery)>,
        P: FnMut() -> Outcome,
        PR: FnMut() -> (PresenceStatus, Option<PresenceDecision>),
        PA: FnOnce() -> PairingReport,
        F: FnOnce() -> String,
        DA: FnOnce() -> String,
        L: FnOnce() -> LightsReport,
        I: FnOnce() -> Result<Vec<ImportFailure>, String>,
        H: FnOnce() -> Result<crate::DeliveryHealth, String>,
        RO: FnOnce() -> Vec<(String, pns_domain::doctor::RouteVerdict)>,
    {
        let checks = self.checks;
        let event = pns_domain::EventArgs {
            agent: "pns".to_string(),
            state: "doctor".to_string(),
            detail: DOCTOR_DETAIL.to_string(),
            ..Default::default()
        };
        let legs: Vec<pns_domain::routing::Leg> = checks
            .iter()
            .filter(|check| check.kind == pns_domain::doctor::CheckKind::Send)
            .map(|check| pns_domain::routing::Leg {
                name: check.plugin,
                // The operator is standing here waiting for the answer, which is
                // what the reporting mode means and which deadline hermes posts
                // under. It decides nothing about who hears the report: the doctor
                // prints every outcome itself.
                mode: pns_domain::routing::ReportMode::ReportOutcome,
                // NOT A DECORATION, because no plan chose these: the doctor
                // bypasses every gate and sends to whatever is enabled. The flag
                // says a leg is there BECAUSE the operator was to be shown
                // something, and the honest answer here is no.
                decorative: false,
            })
            .collect();
        // NO PANE: its only consumer is the banner's click target, and whether a
        // click focuses the right pane cannot be verified without a human clicking
        // it, so carrying one would add the scrub rule to a second call site to
        // test nothing this can observe.
        let delivered = (actions.deliver)(&legs, &event);

        let outcomes: Vec<pns_domain::doctor::Outcome> = checks
            .iter()
            .map(|check| match check.kind {
                pns_domain::doctor::CheckKind::Skipped(reason) => {
                    pns_domain::doctor::Outcome::Skipped(reason)
                }
                pns_domain::doctor::CheckKind::Pulse => (actions.pulse)(),
                // A READING, NEVER A SEND: nothing is dispatched to a sensor, and
                // what an operator cannot see any other way is what it says now.
                pns_domain::doctor::CheckKind::Presence => {
                    let (status, narrowing) = (actions.presence)();
                    pns_domain::doctor::Outcome::Presence(status, narrowing)
                }
                // BY NAME, never by position. The legs above are these checks in
                // this order and `dispatch_legs` answers one outcome per leg, so
                // the two agree today; a positional pairing that ever stopped
                // agreeing would print one channel's verdict under another's
                // label, which is a silent misreport rather than a visible one.
                // The absent case cannot happen and still reports a problem rather
                // than claiming a send, which is the direction to be wrong in.
                pns_domain::doctor::CheckKind::Send => {
                    match delivered.iter().find(|(leg, _)| leg.name == check.plugin) {
                        Some((_, Delivery::Delivered(said))) => {
                            pns_domain::doctor::Outcome::Sent(said.clone())
                        }
                        Some((
                            _,
                            Delivery::Failed(said)
                            | Delivery::Rejected { detail: said, .. }
                            | Delivery::Unlaunched(said),
                        )) => pns_domain::doctor::Outcome::Failed(said.clone()),
                        // Silent BY DESIGN, which is an executable channel that
                        // RAN: it was handed the event and has no second surface
                        // to answer on.
                        Some((_, Delivery::Silent)) => pns_domain::doctor::Outcome::SentUnreported,
                        None => pns_domain::doctor::Outcome::Failed(
                            "the leg was never dispatched".to_string(),
                        ),
                    }
                }
            })
            .collect();

        use pns_domain::doctor::{Item, Mark};
        emit(Item::section("Channels", CHANNELS_BLURB));
        for (check, outcome) in checks.iter().zip(&outcomes) {
            emit(Item::row(
                pns_domain::doctor::outcome_mark(outcome),
                pns_domain::doctor::line(check, outcome),
            ));
        }
        emit(Item::row(
            Mark::Detail,
            pns_domain::doctor::summary(&outcomes),
        ));
        // BETWEEN THE SUMMARY AND THE DECISION SECTION, which is health beside
        // health and history last: this check can move the exit code and the
        // decision log explicitly cannot, so the other order would put a gradeable
        // line below an ungradeable one.
        let pairing = (actions.pairing)();
        emit(Item::section("Pairing", PAIRING_BLURB));
        let pairing_mark = pns_domain::doctor::pairing_mark(&pairing);
        for (index, line) in pns_domain::doctor::pairing_lines(&pairing)
            .into_iter()
            .enumerate()
        {
            // THE FIRST LINE IS THE VERDICT and the rest is what moshi said
            // about it, so only the first carries the grade.
            emit(Item::row(
                if index == 0 {
                    pairing_mark
                } else {
                    Mark::Detail
                },
                line,
            ));
        }
        // GATE STATE ABOVE THE HISTORY THE GATE EXPLAINS, and below the pairing
        // check, which is health. It must NOT move the exit code, for the reason
        // the decision section does not: a Focus being on is not a fault.
        emit(Item::section("Daemon and gates", DAEMON_BLURB));
        emit(Item::note((actions.focus)()));
        // BESIDE THE FOCUS LINE, which is the other line that reports state without
        // grading it. It must NOT move the exit code in any state, including the
        // dead one: a daemon that is down costs ambient features, and this exit
        // code is what an operator's automation reads as "notifications are
        // broken".
        emit(Item::note((actions.daemon)()));
        // IMMEDIATELY BELOW THE DAEMON'S OWN LINE, and that placement is the whole
        // mitigation for the one thing this line does not say: a nag with a dead
        // daemon never fires, and the line above already reports the daemon from its
        // heartbeat. Two lines deriving one fact is how they drift apart, so these
        // two read as one paragraph instead.
        emit(Item::note(pns_domain::doctor::nag_line(
            self.nag_after_secs,
        )));
        // AND THE LAMPS BELOW THE GATE, for the same reason: a dark lamp is not a
        // broken notifier, so this section reports and never grades. It is the last
        // thing that touches the network, so a bridge that hangs cannot delay a
        // line above it.
        emit(Item::section("Lights", LIGHTS_BLURB));
        for line in pns_domain::doctor::lights_lines(&(actions.lamps)()) {
            emit(Item::note(line));
        }
        // APPENDED AFTER THE SUMMARY, which is what lets it be added at all: the
        // census plus its summary is one complete thought whose line order the
        // suite already pins, and nothing below can disturb it.
        emit(Item::section("Delivery", DELIVERY_BLURB));
        for line in crate::delivery_health_lines((actions.delivery_health)()) {
            emit(Item::note(line));
        }
        // IMMEDIATELY UNDER THE LEDGER, because the two answer one question
        // between them: the line above says what is not arriving, and these say
        // whether the gateway would take it if pns sent it again.
        //
        // IT DOES NOT MOVE THE EXIT CODE, and that is not timidity. The roster
        // is derived from what pns has POSTED TO, so a route used once and
        // since retired on the gateway is missing forever with nothing an
        // operator can do to clear it. A check that cannot be satisfied is one
        // they learn to ignore, which would cost them the check that matters.
        // The line says the route is missing in words that cannot be skimmed
        // past; the reader decides.
        let routes = (actions.routes)();
        for (route, verdict) in &routes {
            emit(Item::row(
                pns_domain::doctor::route_mark(verdict),
                pns_domain::doctor::route_line(route, verdict),
            ));
        }
        emit(Item::row(
            Mark::Detail,
            pns_domain::doctor::routes_summary(&routes),
        ));
        emit(Item::section("Recent decisions", DECISIONS_BLURB));
        // THE SECTION CARRIES ITS OWN MARKS NOW. It used to be marked by
        // position here, first row a Note and every other a Detail, which was
        // fine while every entry was exactly one line and wrong the moment a
        // decision grew the sentence explaining it.
        for (mark, line) in
            sections::decision_section(self.records, self.clock.now_secs(), self.decisions)
        {
            emit(Item::row(mark, line));
        }
        // HISTORY BELOW HISTORY, and last for the reason the decision section is
        // second to last: an unreplayed journal is not a failure, so it sits under
        // the one section that already cannot move the exit code.
        emit(Item::section("History", HISTORY_BLURB));
        emit(Item::note(sections::missed_line(
            self.records,
            self.replay_card,
        )));
        // Import failures are recoverable history, not a destination health grade.
        for line in imports::lines((actions.imports)()) {
            emit(Item::row(Mark::Warn, line));
        }
        // THE DECISION SECTION DOES NOT MOVE THE EXIT CODE. It reports HISTORY,
        // not health: an empty log on a fresh machine is not a failure, and
        // neither is one nothing could read. The pairing IS health and does move
        // it, which is why it is an argument rather than a second code combined
        // here: one decision point, decided in one place.
        pns_domain::doctor::exit_code(&outcomes, &pairing)
    }
}

/// What each section's rows are for. THE BLURB IS THE POINT of a heading, not
/// decoration: "Delivery" tells an operator nothing its rows did not already
/// say, and "what has not arrived yet, and whether the gateway would take it"
/// tells them whether to read on.
/// NAMING THE GATES IS THE POINT. The frame's subtitle has room to say they are
/// all bypassed and no room to say which, and knowing which is what tells an
/// operator that a green line here does not promise a green line during an
/// event. A heading's rule is not boxed, so it has the room.
const CHANNELS_BLURB: &str =
    "a real test notification down every channel, ignoring what normally silences one";
const PAIRING_BLURB: &str = "whether the phone that answers cards still knows this Mac";
const DAEMON_BLURB: &str = "what is running, and what would silence a notification";
const LIGHTS_BLURB: &str = "which lamps light, and which states each one answers";
const DELIVERY_BLURB: &str = "what has not arrived, and whether the gateway would take it";
const DECISIONS_BLURB: &str = "why a card did or did not fire, newest first";
const HISTORY_BLURB: &str = "notifications that were never seen, and any this could not read";

/// The payload's detail, so whoever the card wakes knows at once that nothing
/// is wrong and nothing needs doing.
const DOCTOR_DETAIL: &str = "test send from pns doctor; nothing is wrong and nothing needs doing";

mod pulse;
pub use pulse::doctor_pulse;

mod focus;
mod lamps;
pub use focus::{FocusReading, doctor_focus};
pub use lamps::{DoctorBridge, doctor_lamps};

#[cfg(test)]
mod tests;
