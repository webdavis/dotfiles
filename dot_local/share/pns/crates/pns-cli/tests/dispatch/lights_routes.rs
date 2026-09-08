use super::*;

#[test]
fn without_a_lights_table_nothing_new_reaches_the_bridge() {
    // A GUARD, not a red-first test, and it is the compatibility claim of the
    // whole PR: a machine that never wrote a `[lights]` table keeps exactly the
    // pulse it has always had. The long-running event still lights the room,
    // and the blocked turn, which is the new behaviour, does NOT, because the
    // opt-in it needs is the table that is not there.
    let long_running: Vec<&str> = LONG_DONE
        .iter()
        .copied()
        .chain(["--long-running"])
        .collect();
    assert_eq!(
        lamp_run(
            "lamps-no-table-long-done",
            "",
            "",
            &long_running,
            Mute::Nothing,
            Presence::Away
        ),
        (true, true, true, false, Some(0)),
        "the shipped pulse: a long command lights the room and both legs fire"
    );
    assert_eq!(
        lamp_run(
            "lamps-no-table-blocked",
            "",
            "",
            &BLOCKED,
            Mute::Nothing,
            Presence::Away
        ),
        (false, true, true, false, Some(0)),
        "and a blocked turn reaches no bridge at all without the table"
    );
}

#[test]
fn a_blocked_turn_lights_the_lamps_once_the_map_exists() {
    // THE TEST THAT PROVES THE FEATURE EXISTS END TO END. On main the only
    // pulse gate is `plan.pulse`, which is `long_running`, so a blocked agent
    // shows the operator nothing on a bulb however long it waits. With the map
    // written, the blocked lamp is its own gate.
    //
    // WHAT A DIAL CAN PROVE HERE, and the hard limit: the transport is HTTPS
    // with verification disabled and this spy is a plain TCP listener that
    // hangs up, so a binary test can show THAT the bridge was reached and never
    // WHAT was written. Every body, colour and path assertion is a unit test
    // through the `Bridge` trait.
    assert_eq!(
        lamp_run(
            "lamps-map-blocked",
            "",
            STUDIO_MAP,
            &BLOCKED,
            Mute::Nothing,
            Presence::Away
        ),
        (true, true, true, false, Some(0)),
        "the map is written, so a waiting agent reaches the bridge"
    );
}

#[test]
fn an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg() {
    // The shipped whole-pulse property at the new granularity: the lamps are
    // suppressed and NOTHING else is, so the card and the durable log still
    // report a long command at any hour.
    //
    // IT REACHES THE BRIDGE AND WRITES NOTHING, which is the deliberate change.
    // The old pre-resolution gate answered "could any place be awake" from the
    // config alone and paid for it with two stated limits (a lamp that carved an
    // awake window out of a sleeping room lost its signal to the gate, and a
    // claimed light took the house window here and its room's window in the
    // walk). The dim window is now a per-lamp answer that needs the bridge's own
    // membership, so the cheap half of that question no longer exists. What it
    // costs is three GETs on an event fired at an hour every lamp is asleep, and
    // what it buys is that no lamp is ever dark because a gate guessed.
    let asleep = window_around(utc_minute_now(), 120);
    let long_running: Vec<&str> = LONG_DONE
        .iter()
        .copied()
        .chain(["--long-running"])
        .collect();
    assert_eq!(
        lamp_run(
            "lamps-every-place-asleep",
            "",
            &format!(
                "[lights]\nrefresh_secs = 20\n\
                 [lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 dim_window = \"{asleep}\"\ndim_behaviours = []\n"
            ),
            &long_running,
            Mute::Nothing,
            Presence::Away,
        ),
        (true, true, true, false, Some(0)),
        "every lamp asleep: the map still resolves and both legs fire. WHAT A \
         DIAL CAN PROVE HERE stops at the round trip, because this spy is a \
         plain TCP listener that hangs up; that no lamp is WRITTEN to is pinned \
         in the unit tests over dim_showing and pulse_render"
    );
}

#[test]
fn a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing() {
    // `[plugins.hue] quiet_hours` IS NO LONGER A RUNG OF THE ROUTED CHAIN. It
    // is now exactly one thing: the schedule a bare `pns lights quiet` reads,
    // and the window the no-map pulse takes. A routed lamp states its own
    // `dim_window` or has none, so a typo in the house key cannot darken it.
    //
    // THE NO-TABLE SIBLING IS ITS OWN TEST and it still holds:
    // `a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`
    // pins the whole-pulse refusal for a machine that wrote no `[lights]`
    // table, which is the compatibility contract this must not move.
    assert_eq!(
        lamp_run(
            "lamps-house-window-unreadable",
            "quiet_hours = \"10pm-7am\"\n",
            STUDIO_MAP,
            &BLOCKED,
            Mute::Nothing,
            Presence::Away,
        ),
        (true, true, true, false, Some(0)),
        "the routed lamps never consult the house key, so a typo there costs \
         them nothing"
    );
}

#[test]
fn the_operators_own_mute_takes_the_blocked_lamp_with_everything_else() {
    // THE ONE NEW CONDITION `plan.pulse` DOES NOT ALREADY COVER. Arbitration
    // zeroes the plan's pulse for a muted event, so every other lamp in this
    // slice is muted by that alone; the blocked one earns its own gate at the
    // composition root, off the map rather than off the plan, and that gate is
    // the only place the two answers can come out disagreeing about a lamp the
    // operator switched off.
    //
    // TYPED, NOT INJECTED: the mute is armed by running `pns quiet 1h` in the
    // same sandbox, which is the path an operator walks at bedtime.
    assert_eq!(
        lamp_run(
            "lamps-map-blocked-muted",
            "",
            STUDIO_MAP,
            &BLOCKED,
            Mute::Everything,
            Presence::Away
        ),
        (false, false, true, false, Some(0)),
        "muted: no lamp, no card, and the durable log still keeps the event"
    );
    assert_eq!(
        lamp_run(
            "lamps-map-blocked-unmuted",
            "",
            STUDIO_MAP,
            &BLOCKED,
            Mute::Nothing,
            Presence::Away,
        ),
        (true, true, true, false, Some(0)),
        "unmuted control: the same event, the same map, and the lamp lights"
    );
}

#[test]
fn an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone() {
    // A GUARD, and it is the operator's own scope for this command: the lights
    // mute is LIGHTS ONLY. `pns quiet` mutes the engine, this mutes one place's
    // lamps, and nothing reads the other's file. A mute that quietly took the
    // card with it would be the worst version of this feature: an approval the
    // operator is blocked on, silenced by a command about a bedroom lamp.
    //
    // TYPED, NOT INJECTED: the mute is armed by running the subcommand in the
    // same sandbox, which is the path an operator walks at bedtime.
    //
    // THE MUTE IS A RENDER FILTER AT THE PER-LAMP DECISION, decided once, so
    // the map is still resolved and every lamp under the muted name is then
    // written to for nothing. That costs three GETs for the length of the mute
    // and it is what keeps ONE answer to "is this lamp muted": the alternative
    // is a second, config-only copy of the question upstream of the listing,
    // which is how a report and a lamp come to disagree about a muted room.
    assert_eq!(
        lamp_run(
            "lamps-adhoc-quiet-away",
            "",
            STUDIO_MAP,
            &BLOCKED,
            Mute::Lights("3F - Studio"),
            Presence::Away,
        ),
        (true, true, true, false, Some(0)),
        "away: the lamps are quiet and the CARD still reaches the phone"
    );
    // THE BANNER IS OPT IN like every other channel, so the desk runs below
    // switch it on: without its table the surface has nothing to raise and the
    // assertion would pass on a channel that was never enabled.
    let with_banner = format!("[plugins.macos-banner]\nenabled = true\n{STUDIO_MAP}");
    assert_eq!(
        lamp_run(
            "lamps-adhoc-quiet-desk",
            "",
            &with_banner,
            &BLOCKED,
            Mute::Lights("3F - Studio"),
            Presence::Desk,
        ),
        (true, false, true, true, Some(0)),
        "at the desk: the lamps are quiet and the BANNER still runs, with the \
         durable log taking the event either way"
    );
    assert_eq!(
        lamp_run(
            "lamps-adhoc-unmuted-desk",
            "",
            &with_banner,
            &BLOCKED,
            Mute::Nothing,
            Presence::Desk,
        ),
        (true, false, true, true, Some(0)),
        "the unmuted control: the same event at the same desk reaches the \
         bridge too, so the assertions above are about the legs rather than \
         about the dial"
    );
}
