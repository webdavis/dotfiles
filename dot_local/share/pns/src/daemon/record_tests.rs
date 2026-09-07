//! The TAB codec and the shape rules: what a record round-trips as, and what
//! is refused by name before it can ever reach the spool.

use super::{Job, parse, render, validate_shape};

fn full() -> Job {
    Job {
        id: "nag:sess-123".to_string(),
        due: 1_700_000_000,
        until: 1_700_000_300,
        every: Some(30),
        unless_marker: Some("answered-sess-123".to_string()),
        args: vec![
            "--agent".to_string(),
            "pns".to_string(),
            "--detail".to_string(),
            "a nudge with spaces".to_string(),
        ],
    }
}

#[test]
fn a_job_with_every_field_set_round_trips_through_its_on_disk_form() {
    let job = full();
    assert_eq!(parse(&render(&job)), Ok(job));
}

/// Every way a line can fail to be a record, each naming what was wrong.
///
/// A GUESS IS THE FAILURE THIS PREVENTS. A record half-read is a job with
/// a field somebody else's edit decided, and the daemon re-executes this
/// binary from it.
#[test]
fn a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at() {
    let good = render(&full());
    for (case, line, named) in [
        ("empty", String::new(), "empty"),
        (
            "truncated mid-field",
            "id=x\tdue=100\tuntil=200\targ".to_string(),
            "arg",
        ),
        ("a field repeated", format!("{good}\tdue=5"), "due"),
        ("an unknown field", format!("{good}\tbogus=1"), "bogus"),
        (
            "a non-numeric due",
            good.replace("due=1700000000", "due=soon"),
            "due",
        ),
        (
            "a due past u64",
            good.replace("due=1700000000", "due=18446744073709551616"),
            "due",
        ),
        (
            "a negative every",
            good.replace("every=30", "every=-5"),
            "every",
        ),
        (
            "args that are not a list of words",
            good.replace("args=", "args=nonsense args="),
            "args",
        ),
        (
            "a missing required field",
            good.replace("until=1700000300\t", ""),
            "until",
        ),
        (
            "a record past the cap",
            format!("{good}\tmarker={}", "m".repeat(super::RECORD_MAX)),
            "cap",
        ),
    ] {
        let refusal = parse(&line).expect_err(&format!("{case} must be refused"));
        assert!(
            refusal.contains(named),
            "{case}: the refusal must name `{named}`, said: {refusal}"
        );
    }
}

/// THE ONE THAT STOPS A FILENAME BECOMING A PATH. The id is the spool
/// entry's name and the marker is a name joined to the marker directory, so
/// a separator or a parent reference in either writes or reads outside the
/// state directory, silently.
#[test]
fn an_id_cannot_escape_the_spool_directory() {
    let over_long = "x".repeat(super::ID_MAX + 1);
    for (case, id) in [
        ("a parent reference", ".."),
        ("a parent reference inside a name", "a..b"),
        ("a path separator", "a/b"),
        ("a traversal", "../../etc/passwd"),
        ("a leading dot", ".hidden"),
        ("empty", ""),
        ("a control character", "a\u{7}b"),
        ("a newline", "a\nb"),
        ("a space", "a b"),
        ("over-long", over_long.as_str()),
    ] {
        let job = Job {
            id: id.to_string(),
            ..full()
        };
        let refusal = validate_shape(&job).expect_err(&format!("{case} must be refused"));
        assert!(
            refusal.contains("id"),
            "{case}: the refusal must name `id`, said: {refusal}"
        );
    }
    // The ordinary shape a rider will really register.
    assert_eq!(validate_shape(&full()), Ok(()));
    // The marker is a filename by the same road, so it is judged by the
    // same rule and refused under its own name.
    let job = Job {
        unless_marker: Some("../escape".to_string()),
        ..full()
    };
    let refusal = validate_shape(&job).expect_err("a marker that is a path must be refused");
    assert!(
        refusal.contains("marker"),
        "the refusal must name `marker`, said: {refusal}"
    );
}

/// The rest of the registration's refusals, each naming its own field.
///
/// ADDED BEYOND THE BRIEF'S FIFTEEN because the id rule was the only one
/// of the validation set that had a behavior of its own, and an unbounded
/// `every`, a lease that ends before it starts and an unbounded argv are
/// each a way for one registration to cost the daemon or the operator
/// something without ever being refused.
#[test]
fn every_other_out_of_range_field_is_refused_by_name_too() {
    let long_word = "x".repeat(super::ARGS_BYTES_MAX);
    for (case, job, named) in [
        (
            "a repeat faster than the tick",
            Job {
                every: Some(0),
                ..full()
            },
            "every",
        ),
        (
            "a repeat past the ceiling",
            Job {
                every: Some(super::EVERY_MAX_SECS + 1),
                ..full()
            },
            "every",
        ),
        (
            "a lease that ends before it starts",
            Job {
                due: 1_700_000_300,
                until: 1_700_000_299,
                ..full()
            },
            "until",
        ),
        (
            "no argv at all",
            Job {
                args: Vec::new(),
                ..full()
            },
            "args",
        ),
        (
            "more argv words than the cap",
            Job {
                args: vec!["--local-only".to_string(); super::ARGS_MAX + 1],
                ..full()
            },
            "args",
        ),
        (
            "an argv past the byte cap",
            Job {
                args: vec!["--detail".to_string(), long_word.clone()],
                ..full()
            },
            "args",
        ),
    ] {
        let refusal = validate_shape(&job).expect_err(&format!("{case} must be refused"));
        assert!(
            refusal.contains(named),
            "{case}: the refusal must name `{named}`, said: {refusal}"
        );
    }
    // Both edges of the lease are legal: a one-shot whose lease is exactly
    // its due second is the shape the nag registers.
    assert_eq!(
        validate_shape(&Job {
            due: 1_700_000_300,
            until: 1_700_000_300,
            every: None,
            ..full()
        }),
        Ok(())
    );
}

/// The one bound that is a function of the clock, so it lives apart from
/// the shape rules the loop re-applies.
#[test]
fn a_due_outside_a_bounded_window_of_now_is_refused_at_registration() {
    let now = 1_700_000_000;
    for (case, due) in [
        ("far in the future", now + super::DUE_WINDOW_SECS + 1),
        ("far in the past", now - super::DUE_WINDOW_SECS - 1),
    ] {
        let job = Job {
            due,
            until: due + 60,
            ..full()
        };
        let refusal =
            super::validate_registration(&job, now).expect_err(&format!("{case} must be refused"));
        assert!(
            refusal.contains("due"),
            "{case}: the refusal must name `due`, said: {refusal}"
        );
    }
    assert_eq!(super::validate_registration(&full(), now), Ok(()));
}

#[test]
fn the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel() {
    let job = Job {
        every: None,
        unless_marker: None,
        ..full()
    };
    let rendered = render(&job);
    assert!(
        !rendered.contains("every=") && !rendered.contains("marker="),
        "an absent field is not rendered at all: {rendered}"
    );
    assert_eq!(parse(&rendered), Ok(job));
}
