//! The lamps, pinned: the muted places file and what the report says.

use super::fixtures::*;

// --- the ad-hoc quiet ---------------------------------------------------

#[test]
fn a_state_file_that_is_not_epoch_and_place_lines_complains_and_mutes_nothing() {
    // FAIL OPEN AND SAY SO. Every row here is a file this did not write,
    // and the outcome for all of them is the same: no lamp is muted and the
    // operator is told what the file holds, because a mute nobody can see
    // is the state that costs them a notification they were waiting on.
    //
    // THE PADDED ROWS ARE THE POINT. A `trim()` here is the exact leniency
    // that read a padded epoch as a live mute one module over, so a line
    // with a space anywhere it does not belong is refused rather than read.
    for (contents, named) in [
        ("later 3F - Studio\n", "\"later 3F - Studio\""),
        ("-5 3F - Studio\n", "\"-5 3F - Studio\""),
        (" 1000 3F - Studio\n", "\" 1000 3F - Studio\""),
        ("1000  3F - Studio\n", "\"1000  3F - Studio\""),
        ("1000 3F - Studio \n", "\"1000 3F - Studio \""),
        ("1000\n", "\"1000\""),
        ("1000 \n", "\"1000 \""),
        ("1000 3F - Studio\n\n", "\"\""),
        ("\n", "\"\""),
        ("", "\"\""),
    ] {
        assert_eq!(
            muted_entries(contents),
            Err(format!(
                "pns: state error (lights-quiet holds {named}, which is not \
                 an expiry and a place); nothing is quiet, and the next \
                 pns lights quiet write replaces the file"
            )),
            "contents: {contents:?}"
        );
    }
    // AND A FILE PAST THE CAP IS REFUSED WHOLE rather than truncated to it:
    // this command republishes the file every time and drops what expired,
    // so a file this long was written by something else and none of it can
    // be vouched for.
    let past_cap: String = (0..=MAX_MUTED_PLACES)
        .map(|index| format!("1000 room-{index}\n"))
        .collect();
    assert_eq!(
        muted_entries(&past_cap),
        Err(format!(
            "pns: state error (lights-quiet holds {} lines, more than the \
             {MAX_MUTED_PLACES} places it keeps); nothing is quiet, and the \
             next pns lights quiet write replaces the file",
            MAX_MUTED_PLACES + 1
        )),
        "a file past the cap"
    );
    // THE ROUND TRIP, which is what makes every refusal above a refusal of
    // something this never wrote: the place is the rest of the line
    // verbatim, spaces and all, because that is how a room is named.
    assert_eq!(
        muted_entries("1000 3F - Studio\n1800 3F - Master Bedroom\n"),
        Ok(muted(&[
            (1_000, "3F - Studio"),
            (1_800, "3F - Master Bedroom")
        ])),
        "the file this command writes reads back as what it wrote"
    );
    assert_eq!(
        muted_entries("1000 3F - Studio"),
        Ok(muted(&[(1_000, "3F - Studio")])),
        "and the one trailing newline is the only leniency there is"
    );
}

#[test]
fn the_report_names_every_live_place_and_says_so_when_there_are_none() {
    // ROUNDED UP, which is `quiet::status_line`'s own rule reached through
    // its own function: a mute with forty seconds left is still on, and "0
    // minutes" reads as off.
    //
    // AND AN EXPIRED ENTRY IS NOT REPORTED, because the report and the
    // lamps read the same list through the same predicate: a command that
    // said a room was quiet while its lamps were signalling would be worse
    // than saying nothing.
    let now = 1_000;
    assert_eq!(
        muted_report(
            &muted(&[
                (now + 40, "3F - Studio"),
                (now + 1_620, "3F - Master Bedroom")
            ]),
            Some(now)
        ),
        vec![
            "pns lights: `3F - Studio` is quiet for another 1 minute".to_string(),
            "pns lights: `3F - Master Bedroom` is quiet for another 27 minutes".to_string(),
        ]
    );
    assert_eq!(
        muted_report(&muted(&[(now, "3F - Studio")]), Some(now)),
        vec!["pns lights: nothing is quiet".to_string()],
        "an expired entry is not a place to report"
    );
    assert_eq!(
        muted_report(&[], Some(now)),
        vec!["pns lights: nothing is quiet".to_string()],
        "and neither is an empty file"
    );
}

#[test]
fn a_clock_that_will_not_answer_reports_the_reason_never_nothing_is_quiet() {
    // THE ROOT MUTES EVERYTHING ON NO CLOCK (`ad_hoc_quiet`, fail closed),
    // so a report saying "nothing is quiet" here would tell the operator
    // the opposite of what every lamp is about to do.
    assert_eq!(
        muted_report(&[], None),
        vec![
            "pns lights: the clock cannot be read, so no mute can be judged \
             live; every lamp is quiet until it can"
                .to_string()
        ],
        "an empty file with no clock must not read as nothing is quiet"
    );
    assert_eq!(
        muted_report(&muted(&[(1_000, "3F - Studio")]), None),
        vec![
            "pns lights: the clock cannot be read, so no mute can be judged \
             live; every lamp is quiet until it can"
                .to_string()
        ],
        "an entry on file with no clock reports the same, not the entry"
    );
}
