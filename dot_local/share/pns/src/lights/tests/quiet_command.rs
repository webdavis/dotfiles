//! The lamps, pinned: what `pns lights quiet` accepts, refuses and writes.

use super::fixtures::*;
#[test]
fn a_duration_outside_the_bounds_is_refused_by_what_was_typed() {
    // ONE SPELLING OF "HOW LONG" IN THE WHOLE CRATE. The refusal is
    // `parse_duration`'s own, word for word, because a second wording here
    // would be a second set of bounds the day either one moved.
    let known = places(&["3F - Studio"]);
    for typed in ["0s", "25h", "1441m", "9223372036854775807h"] {
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
            Err(format!(
                "pns: quiet duration {typed:?} is outside 1s to 24h"
            )),
            "typed: {typed:?}"
        );
    }
    for typed in ["30", "", "1d", " 5m"] {
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
            Err(format!(
                "pns: quiet duration {typed:?} is not <count><s|m|h>"
            )),
            "typed: {typed:?}"
        );
    }
    assert_eq!(
        quiet_command(&typed_at("3F - Studio", "30m"), &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio".to_string(),
            seconds: 1_800,
        }),
        "and the two ends of the range are what the bounds let through"
    );
}

#[test]
fn a_place_the_config_does_not_name_is_refused_rather_than_silently_stored() {
    // A MUTE IS A LINE NOTHING WILL EVER MATCH. Stored quietly, the lamp
    // the operator meant to quiet goes on flashing while the command
    // reports success, and the only evidence they get is the lamp itself at
    // the hour they were trying not to be disturbed.
    let known = places(&["3F - Studio", "3F - Studio - HCL3"]);
    assert_eq!(
        quiet_command(&typed_at("3F - Nowhere", "30m"), &known, ONE_HOUR),
        Err(
            "pns: lights quiet: \"3F - Nowhere\" is no lamp, room or zone \
             this can quiet; a mute reaches \"3F - Studio\", \
             \"3F - Studio - HCL3\""
                .to_string()
        ),
        "a place nothing in the config names"
    );
    assert_eq!(
        quiet_command(&typed_at("3f - studio", "30m"), &known, ONE_HOUR),
        Err(
            "pns: lights quiet: \"3f - studio\" is no lamp, room or zone \
             this can quiet; a mute reaches \"3F - Studio\", \
             \"3F - Studio - HCL3\""
                .to_string()
        ),
        "and a case-folded one is a typo rather than a name to forgive, \
         which is how the bridge listing reads it too"
    );
    assert_eq!(
        quiet_command(&typed_at("3F - Studio - HCL3", "30m"), &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio - HCL3".to_string(),
            seconds: 1_800,
        }),
        "the control: a lamp the config names is stored"
    );
    assert_eq!(
        quiet_command(&typed_at("3F - Nowhere", "off"), &known, ONE_HOUR),
        Ok(QuietCommand::Unmute {
            place: "3F - Nowhere".to_string(),
        }),
        "and `off` is allowed over any name, because it can only remove: a \
         place muted yesterday and dropped from the config today would \
         otherwise be a mute nothing could clear"
    );
    assert_eq!(
        quiet_command(&[], &known, ONE_HOUR),
        Ok(QuietCommand::Report),
        "no argument reports and mutes nothing"
    );
    assert_eq!(
        quiet_command(
            &typed_at("3F - Studio - HCL1", "30m"),
            &places(&[]),
            ONE_HOUR
        ),
        Err(
            "pns: lights quiet: \"3F - Studio - HCL1\" is no lamp, room or zone \
             this can quiet; this config claims no lamp at all, so there is \
             nothing a mute could reach"
                .to_string()
        ),
        "and a config that claims nothing says so rather than trailing off \
         after `a mute reaches`"
    );
    let arguments = vec![
        "3F - Studio".to_string(),
        "30m".to_string(),
        "x".to_string(),
    ];
    assert_eq!(
        quiet_command(&arguments, &known, ONE_HOUR),
        Err(
            "pns: lights quiet takes a place, optionally with a duration \
             or off, or nothing at all"
                .to_string()
        ),
        "arguments: {arguments:?}"
    );
}

/// A schedule an hour away, which is what a bare mute reads.
const ONE_HOUR: Option<u64> = Some(3_600);

#[test]
fn a_bare_mute_lasts_until_the_operators_quiet_hours_end() {
    let known = places(&["3F - Studio"]);
    assert_eq!(
        quiet_command(&[places(&["3F - Studio"])[0].clone()], &known, ONE_HOUR),
        Ok(QuietCommand::Mute {
            place: "3F - Studio".to_string(),
            seconds: 3_600,
        }),
        "no duration typed: the schedule says how long"
    );
    // NO SCHEDULE IS A REFUSAL, never a guessed length: picking one would be
    // a mute the operator did not ask for, ending at an hour they cannot
    // predict.
    assert_eq!(
        quiet_command(&places(&["3F - Studio"]), &known, None),
        Err(
            "pns: lights quiet: a bare mute lasts until your quiet hours end, \
             and `[plugins.hue] quiet_hours` states none; give a duration \
             instead, or set that key"
                .to_string()
        ),
    );
    // AND AN UNKNOWN PLACE IS STILL REFUSED BY NAME on the bare form, which
    // is the same order the two-word form checks in: a typo must not become
    // a mute nothing will ever match.
    assert_eq!(
        quiet_command(&places(&["3F - Nowhere"]), &known, ONE_HOUR),
        Err(unmutable_sentence("3F - Nowhere", &known)),
    );
}

#[test]
fn how_long_a_bare_mute_runs_is_the_minutes_from_now_to_the_windows_end() {
    // 22:00 to 07:00, which is the window every room in the operator's own
    // config carries.
    const ENDS_AT_0700: Option<u16> = Some(7 * 60);
    assert_eq!(
        bare_mute_secs(ENDS_AT_0700, Some(23 * 60)),
        Some(8 * 3_600),
        "typed at 23:00, the mute runs to 07:00: eight hours over midnight"
    );
    assert_eq!(
        bare_mute_secs(ENDS_AT_0700, Some(6 * 60)),
        Some(3_600),
        "and typed at 06:00 it runs one hour, which is the rest of the window"
    );
    assert_eq!(
        bare_mute_secs(ENDS_AT_0700, Some(15 * 60)),
        Some(16 * 3_600),
        "typed outside the window it still runs to the next end, which is what \
         `until my quiet hours end` says"
    );
    // NOW AT THE END MINUTE IS A WHOLE DAY, not nothing: the window ends
    // this second, so the next end is tomorrow's, and a mute of zero seconds
    // is not a mute.
    assert_eq!(bare_mute_secs(ENDS_AT_0700, Some(7 * 60)), Some(24 * 3_600));
    assert_eq!(
        bare_mute_secs(None, Some(23 * 60)),
        None,
        "no schedule is no bare mute"
    );
    assert_eq!(
        bare_mute_secs(ENDS_AT_0700, None),
        None,
        "and neither is a clock this run cannot read"
    );
    // IT NEVER EXCEEDS THE DURATION CAP the typed form is held to, which is
    // what keeps one command from having two sets of bounds.
    assert!(bare_mute_secs(ENDS_AT_0700, Some(7 * 60 + 1)) <= Some(24 * 3_600));
}

/// The refusal `quiet_command` gives for a place nothing names, so a test
/// asserting it does not restate the sentence.
fn unmutable_sentence(place: &str, known: &[String]) -> String {
    match quiet_command(&places(&[place]), known, Some(1)) {
        Err(said) => said,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn places(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

fn typed_at(place: &str, word: &str) -> Vec<String> {
    vec![place.to_string(), word.to_string()]
}

#[test]
fn a_mute_past_the_places_the_file_keeps_is_refused_rather_than_written() {
    // THE COMMAND MUST NOT PUBLISH A FILE ITS OWN READER REFUSES WHOLE.
    // `muted_entries` rejects a file past the cap and mutes NOTHING, so one
    // line over would cancel every mute on the machine at the next event,
    // silently, at the hour the operator was trying not to be disturbed.
    let full: Vec<Muted> = (0..MAX_MUTED_PLACES)
        .map(|which| Muted {
            expiry: 9_000,
            place: format!("3F - Room {which}"),
        })
        .collect();
    assert_eq!(
        muted_after(&full, "3F - One More", Some(9_000), Some(1_000)),
        Err(
            "pns: lights quiet: 32 places are already quiet, which is every \
             line lights-quiet keeps; the mute was not set, and `pns lights \
             quiet <place> off` ends one"
                .to_string()
        ),
        "a full file plus one more place is a file the reader refuses whole"
    );
    assert_eq!(
        muted_after(&full, "3F - Room 0", Some(9_500), Some(1_000)).map(|kept| kept.len()),
        Ok(MAX_MUTED_PLACES),
        "the control: re-muting a place already in the file replaces its \
         line and never reaches the cap"
    );
    assert_eq!(
        muted_after(&full, "3F - Room 0", None, Some(1_000)).map(|kept| kept.len()),
        Ok(MAX_MUTED_PLACES - 1),
        "and `off` can only shrink it, so it is never refused"
    );
    assert_eq!(
        muted_after(&full, "3F - One More", Some(9_500), Some(9_500)).map(|kept| kept.len()),
        Ok(1),
        "and a file of entries that have all expired is pruned before the \
         cap is asked about, which is what keeps a machine muting a \
         different room every night off this refusal"
    );
}

#[test]
fn off_clears_one_place_and_leaves_the_others_where_they_were() {
    // THE WHOLE FILE IS REPUBLISHED EVERY TIME, so "leaves the others" is
    // the property that has to be pinned: a rewrite that dropped a sibling
    // would be a mute the operator set and can no longer see, which is the
    // silent state this path refuses everywhere else.
    let entries = muted(&[(2_000, "3F - Studio"), (3_000, "3F - Master Bedroom")]);
    assert_eq!(
        muted_after(&entries, "3F - Studio", None, Some(1_000)),
        Ok(muted(&[(3_000, "3F - Master Bedroom")])),
        "off takes the place it names and nothing else"
    );
    assert_eq!(
        muted_after(&entries, "3F - Nowhere", None, Some(1_000)),
        Ok(entries.clone()),
        "and off over a place the file does not hold changes nothing"
    );
    assert_eq!(
        muted_after(&entries, "3F - Studio", Some(9_000), Some(1_000)),
        Ok(muted(&[
            (3_000, "3F - Master Bedroom"),
            (9_000, "3F - Studio")
        ])),
        "a second mute over one place REPLACES its expiry rather than \
         adding a second line for it"
    );
    // THE PRUNE, and it is a bug fix rather than tidiness: the file has a
    // line cap, so a machine that mutes a different room every night would
    // otherwise reach it and have the whole file refused.
    assert_eq!(
        muted_after(
            &muted(&[(500, "3F - Studio"), (3_000, "3F - Master Bedroom")]),
            "3F - Kitchen",
            Some(9_000),
            Some(1_000)
        ),
        Ok(muted(&[
            (3_000, "3F - Master Bedroom"),
            (9_000, "3F - Kitchen")
        ])),
        "an entry that expired is dropped as the file goes past it"
    );
    assert_eq!(
        muted_after(
            &muted(&[(500, "3F - Studio"), (3_000, "3F - Master Bedroom")]),
            "3F - Kitchen",
            None,
            None
        ),
        Ok(muted(&[
            (500, "3F - Studio"),
            (3_000, "3F - Master Bedroom")
        ])),
        "but a clock nobody can read judges nothing, so `off` over a place \
         the file does not hold erases none of it"
    );
    // AND THE ROUND TRIP: what this writes is what the reader reads.
    let kept =
        muted_after(&entries, "3F - Studio", Some(9_000), Some(1_000)).expect("under the cap");
    assert_eq!(
        muted_entries(&format!("{}\n", render_muted(&kept))),
        Ok(kept),
        "the file this writes parses back as the entries it wrote"
    );
}

#[test]
fn an_ad_hoc_quiet_ends_on_the_second_it_names_and_an_expired_file_mutes_nothing() {
    // HALF OPEN, AND THE BOUNDARY SECOND ITSELF is the assertion: a `<=`
    // here is an off-by-one nobody sees, because both neighbours agree
    // under either spelling. It is `quiet::is_muted`'s own edge, asked
    // through this reader so the two cannot come out disagreeing.
    let entries = muted(&[(1_000, "3F - Studio")]);
    assert_eq!(
        muted_places(&entries, Some(999)),
        vec!["3F - Studio".to_string()],
        "the second before the expiry is still quiet"
    );
    assert_eq!(
        muted_places(&entries, Some(1_000)),
        Vec::<String>::new(),
        "and the expiry second itself is already over"
    );
    assert_eq!(
        muted_places(&entries, Some(1_001)),
        Vec::<String>::new(),
        "as is every second after it"
    );
    // A WHOLE FILE OF EXPIRED ENTRIES MUTES NOTHING, which is the state a
    // machine that ran the command yesterday wakes up in: the file is
    // still there and every lamp is loud again.
    assert_eq!(
        muted_places(
            &muted(&[(1_000, "3F - Studio"), (900, "3F - Master Bedroom")]),
            Some(1_000)
        ),
        Vec::<String>::new(),
        "an expired file mutes nothing at all"
    );
    // AND A CLOCK NOBODY CAN READ MUTES NOTHING, which is `is_muted`'s own
    // fail-open direction: a lights mute nobody can see is the dangerous
    // state, so an unreadable clock leaves every lamp loud.
    assert_eq!(
        muted_places(&entries, None),
        Vec::<String>::new(),
        "and a clock this run cannot read mutes nothing"
    );
}
