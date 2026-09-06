//! The decision, pinned: readings.

use super::fixtures::*;

// --- the readings the decision ran on ------------------------------------

#[test]
fn a_decision_reports_the_readings_its_surface_was_decided_from() {
    // THE RECORD IS THE READINGS THIS DECISION RAN ON, never a second
    // reading taken afterwards. Two readings of where the operator is can
    // disagree, and an explanation taken from the later one belongs to a
    // moment the decision never saw.
    let probes = CountingProbes {
        idle: Some(30),
        marker_mtime: Some(999_400),
        phone_atime: Some(999_912),
        screen_locked: Some(false),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let inputs = decide_with(&probes, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.desk_input_age, Some(30));
    assert_eq!(
        inputs.phone_input_age,
        Some(88),
        "aged against the one clock read"
    );
    assert_eq!(inputs.marker_age, Some(600), "aged against that same read");
    assert_eq!(inputs.screen_locked, Some(false));
    assert_eq!(inputs.desk_fresh_secs, Some(DEFAULT_DESK_IDLE_SECS));
    // THE CLOCK ITSELF, and not only the ages taken against it. The two
    // above are aged inside the surface reading, so a decision that
    // carried out no clock at all still reports them; the epoch every
    // recorded line leads with comes from THIS field, and a `None` here
    // dates every entry `-` while the ages beside it look measured.
    assert_eq!(
        inputs.now_secs,
        Some(1_000_000),
        "the one clock read, carried out on the decision it was read for"
    );
    assert_eq!(
        inputs.surface,
        Surface::Desk,
        "and the verdict those readings produced"
    );
}

#[test]
fn a_decision_reports_both_the_sessions_visibility_and_the_one_the_plan_ran_on() {
    // DRILL D6 THROUGH THE RECORD. A Back Tap with moshi closed rewrites a
    // session-reported Visible to Hidden, and a record carrying only the
    // rewritten answer says the session hid the pane when the session said
    // the opposite. Both are kept, so the rewrite is visible as itself
    // rather than only in the card it produced.
    let tapped = CountingProbes {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let inputs = decide_with(&tapped, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.surface, Surface::Mobile);
    assert_eq!(inputs.session_visibility, Visibility::Visible);
    assert_eq!(inputs.visibility, Visibility::Hidden, "the D6 rewrite");

    // D5: moshi open on the pane, where the rewrite must never reach, so
    // the two answers agree and the difference above is the rewrite alone.
    let watching_it = CountingProbes {
        idle: Some(9_000),
        phone_atime: Some(999_990),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let inputs = decide_with(&watching_it, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.session_visibility, Visibility::Visible);
    assert_eq!(inputs.visibility, Visibility::Visible);
}

#[test]
fn a_decision_reports_the_plan_it_arbitrated_and_not_the_matrix_it_started_from() {
    // THE ARBITRATED PLAN IS THE VERDICT. The matrix would banner this
    // event and the long-running tier would pulse it; the operator's mute
    // is applied after both, and a record carrying the matrix's answer
    // would explain a card that never arrived by describing one that was
    // planned.
    let probes = || CountingProbes {
        idle: Some(2),
        view: Some(elsewhere("wW:p1")),
        ..CountingProbes::default()
    };
    let long_event = |overrides: &Overrides| {
        decide(
            &probes(),
            &three_selection(),
            overrides,
            false,
            false,
            "wW:p1",
            Some(1_000_000),
            true,
            false,
        )
        .plan
    };
    assert_eq!(
        long_event(&Overrides::default()),
        DeliveryPlan {
            banner: true,
            phone_card: false,
            pulse: true,
        },
        "unmuted control: the matrix's own answer"
    );
    assert_eq!(
        long_event(&Overrides {
            muted: true,
            ..Overrides::default()
        }),
        DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        }
    );
}

#[test]
fn a_reading_nobody_could_take_is_reported_as_absent_and_never_as_a_number() {
    // AN ABSENCE IS NOT A ZERO. Every field here is an `Option` precisely
    // so an unread probe stays unread in the record: a `0` would read as
    // "touched this instant" and a `false` lock would read as "the screen
    // was awake", each of which explains a decision by an observation
    // nobody made.
    let all_readable = CountingProbes {
        idle: Some(30),
        marker_mtime: Some(999_400),
        phone_atime: Some(999_912),
        screen_locked: Some(true),
        ..CountingProbes::default()
    };

    // A GARBLED THRESHOLD: there is no window, so nothing below it was
    // measured either.
    let garbled = Overrides::from_env(&BTreeMap::from([(
        "PNS_DESK_IDLE_SECS".to_string(),
        "0600".to_string(),
    )]));
    let inputs = decide_with(&all_readable, &garbled, "").inputs;
    assert_eq!(inputs.desk_fresh_secs, None, "no window to measure against");
    assert_eq!(inputs.desk_input_age, None);
    assert_eq!(inputs.phone_input_age, None);
    assert_eq!(inputs.marker_age, None);
    assert_eq!(inputs.screen_locked, None);

    // AN UNREADABLE CLOCK ages nothing, so neither phone signal has an
    // age, while the desk clock, which is an age already, still does.
    let inputs = decide(
        &all_readable,
        &three_selection(),
        &Overrides::default(),
        false,
        false,
        "",
        None,
        false,
        false,
    )
    .inputs;
    assert_eq!(inputs.phone_input_age, None, "aged against no clock");
    assert_eq!(inputs.marker_age, None, "aged against no clock");
    assert_eq!(inputs.desk_input_age, Some(30));

    // AN UNREAD LOCK is neither locked nor unlocked. The probe is skipped
    // wherever the idle clock answered nothing, which is exactly where a
    // `false` would claim a display somebody was sitting at.
    let no_idle_reading = CountingProbes {
        idle: None,
        screen_locked: Some(true),
        ..CountingProbes::default()
    };
    let inputs = decide_with(&no_idle_reading, &Overrides::default(), "").inputs;
    assert_eq!(inputs.screen_locked, None);
    assert_eq!(no_idle_reading.lock_reads.get(), 0, "and never read at all");
}

#[test]
fn writing_the_record_consults_no_probe_the_decision_had_not_already_read() {
    // THE RECORD MUST NOT BECOME A SECOND READING. The whole feature is
    // worthless, and actively misleading, if any value on the line is
    // re-read after `decide` returned: two readings of where the operator
    // is can disagree, and the explanation would then belong to a moment
    // the decision never saw. AN EXTRA READ IS A FAILURE EVEN WHERE THE
    // VALUE HAPPENS TO MATCH, which is why this compares the counts rather
    // than the line.
    let reads = |also_record: bool| {
        let probes = CountingProbes {
            idle: Some(30),
            marker_mtime: Some(999_400),
            phone_atime: Some(999_912),
            screen_locked: Some(false),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        if also_record {
            crate::decision_log::line(&crate::decision_log::Record {
                event: &crate::args::EventArgs::default(),
                decision: &decision,
                overrides: &Overrides::default(),
                legs: &[],
                nag: false,
                permission_mode: "",
                agent_id: "",
                tool_name: "",
            });
        }
        [
            probes.idle_reads.get(),
            probes.marker_reads.get(),
            probes.phone_reads.get(),
            probes.lock_reads.get(),
            probes.view_reads.get(),
        ]
    };
    assert_eq!(reads(true), reads(false));
    // And every probe really was consulted, so the equality above is an
    // agreement between two live readings rather than between two zeroes.
    assert_eq!(reads(false), [1, 1, 1, 1, 1]);
}
