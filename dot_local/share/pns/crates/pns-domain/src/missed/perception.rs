use crate::decision::{Decision, Overrides};
use crate::routing::{Delivery, Leg};
use crate::surface::{Surface, Visibility};

/// Whether no acknowledged decoration reached an operator who was not watching.
/// The routed leg carries its decorative role; names and report mode are not policy.
/// A durable log alone, an unlaunched or failed send, and an unconfirmed silent
/// executable leave a miss. Lights are decoration of state, not acknowledgement.
/// The existing watching and skip-phone exclusions still describe perception
/// outside this dispatch. An empty desk's visible pane does not count as watched.
pub fn was_missed(
    decision: &Decision,
    overrides: &Overrides,
    outcomes: &[(Leg, Delivery)],
) -> bool {
    let watching = decision.inputs.visibility == Visibility::Visible
        && decision.inputs.surface != Surface::Away;
    let acknowledged = outcomes
        .iter()
        .any(|(leg, outcome)| leg.decorative && matches!(outcome, Delivery::Delivered(_)));
    !overrides.skip_phone && !watching && !acknowledged
}
/// Whether this event is the operator's RETURN, and so the moment a queued
/// notification can be put in front of them.
///
/// THE RETURN TRANSITION IS THE NEXT EVENT, and the engine has already
/// computed it. Nothing schedules a probe, so nothing OBSERVES a transition;
/// what the engine does do is read presence per event, at the last moment
/// before delivery, and publish the answer as the plan and the surface it
/// decided on. Both clauses below are values the record site already holds, so
/// this is no new probe, no second reading and no new trigger, and it inherits
/// the timing ruling for free.
///
/// AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED. The Away
/// row always cards, so without this clause the journal would be flushed at
/// the phone of an operator who has not come back, which is the opposite of
/// what "return" means.
///
/// The plan chooses when replay may be attempted. A failed live decoration
/// can still be missed; the replay operation owns its durable acknowledgement
/// and retry policy. A mute or an undecorated plan does not attempt replay.
///
/// IT IS THE ENGINE'S OWN PERCEPTION RULE RESTATED, not a second one. An
/// operator at the desk watching the origin pane earns nothing, live or
/// replayed, so the queue waits for an event on a pane they are not watching.
pub fn should_replay(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away && (decision.plan.banner || decision.plan.phone_card)
}
/// Whether this event PROVES the operator was here, and so moves the recap
/// window's near edge forward.
///
/// AWAY IS THE ONLY THING THAT DOES NOT COUNT. Desk and Mobile are both a
/// human within reach of a screen; Away is the state the whole recap exists to
/// bracket, and the window it brackets runs from the last event that was not
/// one to now.
///
/// VISIBILITY IS DELIBERATELY NOT READ, unlike `was_missed`'s watching clause.
/// An operator at the desk looking at a different pane is still present, and
/// reading visibility here would make the window's near edge depend on which
/// pane happened to fire.
///
/// IT READS A VALUE THE DECISION ALREADY HOLDS, so it is no new probe and no
/// second reading, exactly as `was_missed` and `should_replay` are argued
/// above.
pub fn is_present(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away
}
