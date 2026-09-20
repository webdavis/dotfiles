use super::*;

fn identity() -> SubmissionIdentity {
    SubmissionIdentity {
        producer: "pns-return".into(),
        request_id: "deadbeef".into(),
    }
}

/// THE MUTANT THIS PINS: any field dropped from the line, or a mode or
/// decorative flag flattened to a constant, which would send the card to the
/// right destination in the wrong mode under an identity nothing dedupes.
#[test]
fn a_handed_card_survives_the_pipe_unchanged() {
    let legs = [
        Leg {
            name: "mobile",
            mode: ReportMode::Silent,
            decorative: true,
        },
        Leg {
            name: "hermes",
            mode: ReportMode::ReportOutcome,
            decorative: false,
        },
    ];
    let identity = identity();
    let read = decode_handed_card(&encode(&ReplayCard {
        identity: &identity,
        detail: "2 events. recap in #pns",
        legs: &legs,
    }))
    .expect("a card this wrote must read back");
    assert_eq!(read.identity, identity);
    assert_eq!(read.detail, "2 events. recap in #pns");
    assert_eq!(read.legs, legs);
}

#[test]
fn a_pipe_closed_without_a_card_is_no_card() {
    assert!(decode_handed_card("").is_none());
    assert!(decode_handed_card("\n").is_none());
}

#[test]
fn a_line_this_cannot_vouch_for_is_no_card() {
    assert!(decode_handed_card("{").is_none());
    assert!(decode_handed_card(r#"{"producer":"pns-return","detail":"x","legs":[]}"#).is_none());
}

/// A leg no plugin registered cannot be dispatched, and must not take the
/// rest of the card with it.
#[test]
fn a_leg_named_by_nothing_registered_is_skipped() {
    let identity = identity();
    let legs = [
        Leg {
            name: "nowhere",
            mode: ReportMode::Silent,
            decorative: true,
        },
        Leg {
            name: "banner",
            mode: ReportMode::Silent,
            decorative: true,
        },
    ];
    let read = decode_handed_card(&encode(&ReplayCard {
        identity: &identity,
        detail: "1 missed notification",
        legs: &legs,
    }))
    .expect("the card still stands");
    assert_eq!(read.legs, [legs[1]]);
}
