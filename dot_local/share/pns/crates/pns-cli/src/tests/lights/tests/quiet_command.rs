//! The lamps, pinned: what `pns lights quiet` accepts, refuses and writes.

use super::fixtures::*;
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
