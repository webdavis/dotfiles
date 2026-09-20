use super::*;

#[test]
fn no_lights_table_is_none_and_an_empty_one_is_every_locked_default() {
    // ABSENT AND EMPTY ARE DIFFERENT CONFIGS, which is what the Option
    // spells: a machine with no table keeps the room-based pulse it has
    // always had, and a machine with an empty one has asked for the lamps
    // and routed nothing yet.
    assert_eq!(parse_config("").expect("empty parses").lights, None);
    let shipped = lights("[lights]\n");
    assert_eq!(shipped, Lights::default());
    // THE LOCKED FIGURES, each one set on a real lamp. A change to any of
    // them is a change to something that was looked at.
    assert_eq!(shipped.arm_interval_secs, 12);
    assert_eq!(
        shipped.done,
        Pulse {
            duration_ms: 4000,
            brightness: 100
        }
    );
    assert_eq!(shipped.failed, shipped.done);
    assert_eq!(
        shipped.blocked,
        Blocked {
            breath: Breath {
                duration_ms: 2000,
                high: 100,
                low: 30
            },
            lease_expiry_secs: 57_600,
        }
    );
    assert_eq!(
        shipped.unseen,
        Unseen {
            breath: Breath {
                duration_ms: 4000,
                high: 60,
                low: 10
            },
            arm_after_secs: 300,
        }
    );
    assert_eq!(
        shipped.looping,
        Looping {
            breathe_then_flare: BreatheThenFlare {
                breath: Breath {
                    duration_ms: 4000,
                    high: 80,
                    low: 10
                },
                flare: 100,
                flare_ms: 200,
            },
            arm_after_secs: 300,
            lease_expiry_secs: 3900,
        }
    );
    assert_eq!(
        shipped.dim,
        Breath {
            duration_ms: 3000,
            high: 7,
            low: 1
        }
    );
    assert!(shipped.lamps.is_empty() && shipped.rooms.is_empty() && shipped.zones.is_empty());
}

#[test]
fn a_behaviour_table_moves_the_keys_it_states_and_leaves_the_rest_at_their_locked_values() {
    let stated = lights(
        "[lights]\narm_interval = \"25s\"\n\
             [lights.done]\nduration = \"1500ms\"\n\
             [lights.blocked]\nlow_percent = 45\n\
             [lights.unseen]\narm_after = \"60s\"\n\
             [lights.loop]\narm_after = \"6m\"\nlease_expiry = \"10m\"\n\
             [lights.dim]\nhigh_percent = 9\n",
    );
    assert_eq!(stated.arm_interval_secs, 25);
    assert_eq!(
        stated.done,
        Pulse {
            duration_ms: 1500,
            brightness: 100
        },
        "the duration moved and the brightness stayed at its locked value"
    );
    assert_eq!(
        stated.failed,
        Lights::default().failed,
        "and its sibling is untouched"
    );
    assert_eq!(stated.blocked.breath.low, 45);
    assert_eq!(stated.blocked.breath.duration_ms, 2000);
    assert_eq!(
        stated.blocked.lease_expiry_secs,
        Lights::default().blocked.lease_expiry_secs,
        "the breath moved and the backstop stayed at its locked default"
    );
    assert_eq!(stated.unseen.arm_after_secs, 60);
    assert_eq!(stated.unseen.breath, Lights::default().unseen.breath);
    assert_eq!(stated.looping.arm_after_secs, 360);
    assert_eq!(stated.looping.lease_expiry_secs, 600);
    assert_eq!(stated.dim.high, 9);
    assert_eq!(stated.dim.low, 1);
}

#[test]
fn a_knob_that_does_not_apply_to_a_behaviour_does_not_exist_on_it() {
    // NO DEAD KNOBS (operator ruling), enforced by the roster rather than by
    // a comment: a blink has no low end to fade to, and a breath has no
    // single brightness. A reader who sets one and watches nothing happen is
    // exactly what this refuses.
    for (written, key) in [
        ("[lights.done]\nlow_percent = 10\n", "low"),
        ("[lights.done]\nhigh_percent = 90\n", "high"),
        ("[lights.failed]\nlow_percent = 10\n", "low"),
        ("[lights.blocked]\nbrightness_percent = 90\n", "brightness"),
        ("[lights.unseen]\nbrightness_percent = 90\n", "brightness"),
        ("[lights.loop]\nbrightness_percent = 90\n", "brightness"),
        ("[lights.dim]\nbrightness_percent = 90\n", "brightness"),
        ("[lights.dim]\narm_after = \"90s\"\n", "arm_after"),
        ("[lights.done]\narm_after = \"90s\"\n", "arm_after"),
        ("[lights.blocked]\narm_after = \"90s\"\n", "arm_after"),
        ("[lights.unseen]\nlease_expiry = \"90s\"\n", "lease_expiry"),
    ] {
        let said = refusal(written);
        assert!(
            said.contains(key) && said.contains("the table serves"),
            "{written:?} must refuse `{key}` by name and list what the table does \
                 serve: {said}"
        );
    }
}
