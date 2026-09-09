use super::{CARD, NOTHING, Surface, Visibility, decided, was_missed};
use crate::Overrides;
use crate::routing::{Delivery, Leg, ReportMode};

fn leg(decorative: bool) -> Leg {
    Leg {
        name: "arbitrary-destination",
        mode: ReportMode::Silent,
        decorative,
    }
}

#[test]
fn failed_unlaunched_and_unconfirmed_decorations_remain_missed() {
    let decision = decided(Surface::Away, Visibility::Hidden, CARD);
    for outcome in [
        Delivery::Failed("refused".into()),
        Delivery::Unlaunched("absent".into()),
        Delivery::Silent,
    ] {
        assert!(was_missed(
            &decision,
            &Overrides::default(),
            &[(leg(true), outcome)]
        ));
    }
}

#[test]
fn actual_acknowledgement_counts_even_when_the_plan_promised_nothing() {
    let decision = decided(Surface::Desk, Visibility::Hidden, NOTHING);
    assert!(!was_missed(
        &decision,
        &Overrides::default(),
        &[(leg(true), Delivery::Delivered("accepted".into()))]
    ));
}

#[test]
fn a_durable_acknowledgement_alone_does_not_prevent_a_miss() {
    let decision = decided(Surface::Away, Visibility::Hidden, CARD);
    assert!(was_missed(
        &decision,
        &Overrides::default(),
        &[(leg(false), Delivery::Delivered("logged".into()))]
    ));
}

#[test]
fn one_acknowledged_decoration_among_failures_prevents_a_miss() {
    let decision = decided(Surface::Away, Visibility::Hidden, CARD);
    let outcomes = [
        (leg(true), Delivery::Failed("first".into())),
        (leg(true), Delivery::Delivered("second".into())),
        (leg(true), Delivery::Silent),
    ];
    assert!(!was_missed(&decision, &Overrides::default(), &outcomes));
}

#[test]
fn a_pulse_without_an_acknowledged_card_is_still_missed() {
    let mut decision = decided(Surface::Away, Visibility::Hidden, NOTHING);
    decision.plan.pulse = true;
    assert!(was_missed(&decision, &Overrides::default(), &[]));
}
