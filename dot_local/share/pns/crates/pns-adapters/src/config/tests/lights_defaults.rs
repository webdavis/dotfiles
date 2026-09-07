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
    assert_eq!(shipped.refresh_secs, 12);
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
            give_up_after_secs: 57_600,
        }
    );
    assert_eq!(
        shipped.unread,
        Unread {
            breath: Breath {
                duration_ms: 4000,
                high: 60,
                low: 10
            },
            after_secs: 300,
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
            threshold_secs: 300,
            lease_timeout_secs: 3900,
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
        "[lights]\nrefresh_secs = 25\n\
             [lights.done]\nduration_ms = 1500\n\
             [lights.blocked]\nlow = 45\n\
             [lights.unread]\nafter_secs = 60\n\
             [lights.loop]\nthreshold_secs = 360\nlease_timeout_secs = 600\n\
             [lights.dim]\nhigh = 9\n",
    );
    assert_eq!(stated.refresh_secs, 25);
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
        stated.blocked.give_up_after_secs,
        Lights::default().blocked.give_up_after_secs,
        "the breath moved and the backstop stayed at its locked default"
    );
    assert_eq!(stated.unread.after_secs, 60);
    assert_eq!(stated.unread.breath, Lights::default().unread.breath);
    assert_eq!(stated.looping.threshold_secs, 360);
    assert_eq!(stated.looping.lease_timeout_secs, 600);
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
        ("[lights.done]\nlow = 10\n", "low"),
        ("[lights.done]\nhigh = 90\n", "high"),
        ("[lights.failed]\nlow = 10\n", "low"),
        ("[lights.blocked]\nbrightness = 90\n", "brightness"),
        ("[lights.unread]\nbrightness = 90\n", "brightness"),
        ("[lights.loop]\nbrightness = 90\n", "brightness"),
        ("[lights.dim]\nbrightness = 90\n", "brightness"),
        ("[lights.dim]\nthreshold_secs = 90\n", "threshold_secs"),
        ("[lights.done]\nthreshold_secs = 90\n", "threshold_secs"),
        ("[lights.blocked]\nafter_secs = 90\n", "after_secs"),
        (
            "[lights.unread]\nlease_timeout_secs = 90\n",
            "lease_timeout_secs",
        ),
    ] {
        let said = refusal(written);
        assert!(
            said.contains(key) && said.contains("the table serves"),
            "{written:?} must refuse `{key}` by name and list what the table does \
                 serve: {said}"
        );
    }
}
