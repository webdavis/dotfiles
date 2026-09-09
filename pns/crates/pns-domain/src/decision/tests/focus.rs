use super::fixtures::{decide, decide_with, elsewhere, names, three_selection};
use crate::{EnvironmentSnapshot, Overrides};

#[test]
fn a_focus_the_config_named_suppresses_the_mutes_three_decorations_and_beats_a_forced_phone() {
    // THE OPERATING SYSTEM'S MUTE takes the operator's own mute's seat, so
    // it suppresses the same three decorations, applies at the same point
    // (after the skip-beats-force arbitration) and leaves the durable log
    // alone for the same structural reason.
    //
    // A WORLD THAT PLANS ALL THREE: at the desk with the origin pane out
    // of sight earns the banner, `force_phone` earns the card, and a long
    // running event earns the pulse. Anything less and a passing assertion
    // would be a plan that was empty to begin with.
    let world = |overrides: &Overrides| {
        decide(
            &EnvironmentSnapshot {
                idle: Some(2),
                view: Some(elsewhere("wW:p1")),
                ..EnvironmentSnapshot::default()
            },
            &three_selection(),
            overrides,
            crate::DeliveryScope::Automatic,
            "wW:p1",
            Some(1_000_000),
            true,
            false,
        )
        .plan
    };
    let forced = Overrides {
        force_phone: true,
        ..Overrides::default()
    };
    assert_eq!(
        world(&forced),
        crate::surface::DeliveryPlan {
            banner: true,
            phone_card: true,
            pulse: true,
        },
        "control: unfocused and unmuted, all three decorations fire"
    );
    assert_eq!(
        world(&Overrides {
            focus_active: true,
            muted: false,
            force_phone: true,
            ..Overrides::default()
        }),
        crate::surface::DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        },
        "a Focus a producer can override is not a Focus"
    );
    // THE RECORD SURVIVES, structurally: hermes is not a field of the
    // delivery plan, so the durable log is exempt and a Focus is lossless.
    assert_eq!(
        names(&decide_with(
            &EnvironmentSnapshot {
                idle: Some(2),
                view: Some(elsewhere("wW:p1")),
                ..EnvironmentSnapshot::default()
            },
            &Overrides {
                focus_active: true,
                ..Overrides::default()
            },
            "wW:p1"
        )),
        vec!["hermes"]
    );
    // AND THE MUTE STILL WORKS ALONE, which is what stops the new clause
    // being written as a replacement for the old one rather than beside it.
    assert_eq!(
        world(&Overrides {
            focus_active: false,
            muted: true,
            force_phone: true,
            ..Overrides::default()
        }),
        crate::surface::DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        },
        "the operator's own typed mute is untouched by the Focus clause"
    );
}
