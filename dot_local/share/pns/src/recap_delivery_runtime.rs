use crate::*;

/// One recap posted to one route, and what the route had to say about it.
///
/// IT SAYS WHAT HAPPENED, which is what `ReportMode::ReportOutcome` was for
/// and what it never actually did: `dispatch_legs` RETURNS its outcomes and
/// prints nothing, so the mode only ever moved the deadline. MEASURED against
/// a dead endpoint, `pns recap --since ... --until ...` printed nothing and
/// exited 0, which is exactly the drill an operator runs by hand to check a
/// `pns-recap` route they have just prepared, against exactly the failure it
/// is most likely to meet.
///
/// THE SAME LINE `run_event` PRINTS, prefix and all, because a second spelling
/// of one report is a second thing to keep in step. The detached child's
/// stdout is `/dev/null`, so this costs the event path nothing.
pub(crate) fn deliver_recap(
    body: &str,
    channel: &str,
    home: &str,
    hermes_key: Option<String>,
) -> Vec<(pns::routing::Leg, Delivery)> {
    // ONE LEG AND ONE DESTINATION, built by hand the way `doctor_mode` builds
    // its own: no decision was taken here, so there is no plan to derive legs
    // from. NOT DECORATIVE, because nothing about this was chosen to put
    // something in front of the operator; the card already did that.
    let leg = pns::routing::Leg {
        name: "hermes",
        mode: pns::routing::ReportMode::ReportOutcome,
        decorative: false,
    };
    let event = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "recap".to_string(),
        detail: body.to_string(),
        channel: channel.to_string(),
        ..Default::default()
    };
    // NO MOBILE VERDICT TO CARRY: the one leg is hermes, so the mobile table
    // was never read on this path and the default states exactly that.
    let outcomes = dispatch_legs(&[leg], false, &event, home, &Mobile::default(), hermes_key);
    for (leg, delivered) in &outcomes {
        if let Some(line) = delivered.clone().line_for(leg.mode) {
            println!("pns: {line}");
        }
    }
    outcomes
}
