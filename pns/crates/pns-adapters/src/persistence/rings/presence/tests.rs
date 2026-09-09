use super::{Entry, entry, last, reason_said, recorded};
use pns_domain::home::{DeviceKey, HomePresence};
use pns_domain::{Full, Narrowing, Snapshot};
use pns_domain::{PresenceStatus, Unreadable};

/// The snapshot the record is taken from: the desk warm, the phone home,
/// and a fresh reading in the master bedroom.
fn snapshot() -> Snapshot {
    Snapshot {
        status: PresenceStatus::Room {
            room: "3F - MBedroom".to_string(),
            age_secs: 0,
        },
        desk_idle_secs: Some(4),
        screen_locked: Some(false),
        home: HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        },
        desk_room: Some("3F - Studio".to_string()),
        desk_stale_after_secs: 120,
        now: Some(1_700_000_000),
    }
}

#[test]
fn the_record_carries_the_reading_the_desk_clock_and_the_router_verdict() {
    // All four, because the answer is only explicable from all of them:
    // the same room can be chosen off motion or off the desk, and the same
    // motion can be overruled or obeyed.
    let record = recorded(&snapshot(), &Narrowing::To("3F - MBedroom".to_string()));
    assert_eq!(record.at, Some(1_700_000_000));
    assert_eq!(record.presence, r#"room "3F - MBedroom" (0s ago)"#);
    assert_eq!(record.desk_idle_secs, Some(4));
    assert_eq!(record.home, "home");
    assert_eq!(record.room, Some("3F - MBedroom".to_string()));
    assert_eq!(record.reason, "");
}

#[test]
fn the_router_verdict_is_recorded_as_one_word_and_never_its_evidence() {
    // The matched key and the value it matched are the router
    // diagnostic's business; this record is read for which way the gate
    // went, and the value is the phone's own name.
    for (home, said) in [
        (
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            },
            "home",
        ),
        (HomePresence::NotHome, "not-home"),
        (HomePresence::Unknown, "unknown"),
    ] {
        let record = recorded(
            &Snapshot { home, ..snapshot() },
            &Narrowing::Full(Full::Nowhere),
        );
        assert_eq!(record.home, said);
        assert!(!record.home.contains("mister"), "{}", record.home);
    }
}

#[test]
fn every_way_a_routing_can_be_left_whole_names_its_own_reason() {
    // EACH IS A DIFFERENT THING TO GO AND FIX, so collapsing any two into
    // one wording sends half the readers to the wrong edit.
    let said = [
        (Full::NotHome, "the phone is not on the home network"),
        (
            Full::NoDeskRoom,
            "at the desk, and no desk_room says which room that is",
        ),
        (
            Full::Ambiguous {
                desk: "3F - Studio".to_string(),
                motion: "2F - Kitchen".to_string(),
            },
            r#"the desk in "3F - Studio" and newer motion in "2F - Kitchen" disagree"#,
        ),
        (Full::Nowhere, "motion in no watched room"),
        (
            Full::NoLampIn("3F - Hallway".to_string()),
            r#"no lamp in "3F - Hallway""#,
        ),
        (Full::Unknown(Unreadable::NoReading), "unknown: no reading"),
        (
            Full::Unknown(Unreadable::NoClock),
            "unknown: the clock could not be read",
        ),
        (
            Full::Unknown(Unreadable::Stale { poll_age_secs: 90 }),
            "unknown: stale, poll 90s old",
        ),
        (Full::Unknown(Unreadable::Future), "unknown: future epoch"),
        (
            Full::Unknown(Unreadable::NotWatched),
            "unknown: the reported room is not one this config watches",
        ),
    ];
    for (full, named) in said {
        assert_eq!(reason_said(&full), named, "{full:?}");
    }
}

fn narrowed_to(room: &str) -> Entry {
    Entry {
        at: Some(1_700_000_000),
        presence: "room (0s ago)".to_string(),
        desk_idle_secs: Some(12),
        home: "Home".to_string(),
        room: Some(room.to_string()),
        reason: String::new(),
    }
}

#[test]
fn a_room_name_carrying_the_readers_own_field_marker_still_reads_back_whole() {
    // The name is the bridge's own text. Parsed out of a `key=value` line
    // by splitting on a marker, this name would hand the doctor the tail
    // of its own room name and call it the decision.
    let hostile = narrowed_to("Kitchen narrowed=Studio");
    assert_eq!(last(&entry(&hostile)), Some(hostile));
}

#[test]
fn a_room_name_carrying_a_newline_stays_one_entry() {
    // One entry is one line, or an append forges a second the doctor then
    // reads as the decision.
    let written = entry(&narrowed_to("Kitchen\nforged"));
    assert!(!written.contains('\n'), "{written}");
    assert_eq!(
        last(&written).and_then(|read| read.room),
        Some("Kitchen\nforged".to_string())
    );
}

#[test]
fn a_routing_left_whole_carries_its_reason_and_names_no_room() {
    // The two are told apart structurally rather than by the shape of a
    // phrase, so a room literally named "nothing (away)" cannot read as a
    // fallback.
    let stood = Entry {
        room: None,
        reason: "away".to_string(),
        ..narrowed_to("unused")
    };
    let read = last(&entry(&stood)).expect("the entry reads back");
    assert_eq!((read.room, read.reason), (None, "away".to_string()));
}

#[test]
fn the_newest_entry_is_the_one_read_back() {
    // The ring appends, so the decision the doctor reports is the last
    // line, never the first.
    let ring = format!(
        "{}\n{}\n",
        entry(&narrowed_to("2F - Kitchen")),
        entry(&narrowed_to("3F - Studio"))
    );
    assert_eq!(
        last(&ring).and_then(|read| read.room),
        Some("3F - Studio".to_string())
    );
}

#[test]
fn a_line_this_cannot_read_is_skipped_rather_than_hiding_the_ones_behind_it() {
    // VALID JSON IS NOT A VALID RECORD, and reading it as one is worse
    // than reading nothing: `{}` and a wrong-typed field both parse, and
    // filled in from `Default` they became an entry saying nothing, which
    // stopped the reverse walk here and hid the real decision above.
    for corrupt in [
        "not json at all",
        "{}",
        r#"{"at":1,"desk_idle_secs":2,"home":"home","room":null,"reason":"x"}"#,
        r#"{"at":1,"presence":12,"desk_idle_secs":2,"home":"home","reason":"x"}"#,
        r#"{"at":1,"presence":"p","desk_idle_secs":2,"home":["home"],"reason":"x"}"#,
        r#"{"at":1,"presence":"p","desk_idle_secs":2,"home":"home","room":null}"#,
        // A ROOM THAT IS PRESENT AND IS NOT A STRING. Read as "no room",
        // this line becomes a decision that narrowed nothing, which is a
        // perfectly ordinary record and hides the real one behind it.
        r#"{"at":1,"presence":"p","desk_idle_secs":2,"home":"h","room":12,"reason":"x"}"#,
        r#"{"at":1,"presence":"p","desk_idle_secs":2,"home":"h","room":[],"reason":"x"}"#,
    ] {
        let ring = format!("{}\n{corrupt}\n", entry(&narrowed_to("2F - Kitchen")));
        assert_eq!(
            last(&ring).and_then(|read| read.room),
            Some("2F - Kitchen".to_string()),
            "{corrupt}"
        );
    }
}
