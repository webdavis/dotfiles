use super::*;

#[test]
fn a_bare_name_is_an_override_that_stands_until_cleared() {
    assert_eq!(
        parse_override("night"),
        Some(Override {
            profile: "night".to_string(),
            until: None
        })
    );
    assert_eq!(
        parse_override("night\n"),
        parse_override("night"),
        "one trailing newline"
    );
}

#[test]
fn a_name_and_an_epoch_is_a_bounded_override() {
    assert_eq!(
        parse_override("night 1758420600"),
        Some(Override {
            profile: "night".to_string(),
            until: Some(1_758_420_600)
        })
    );
}

#[test]
fn anything_else_reads_as_no_override_at_all() {
    assert_eq!(parse_override(""), None);
    assert_eq!(parse_override("   "), None);
    assert_eq!(
        parse_override("night tomorrow"),
        None,
        "an expiry is an epoch second"
    );
    assert_eq!(parse_override("night 1758420600 extra"), None);
    assert_eq!(parse_override("night\nwork"), None, "one line, not a list");
}

#[test]
fn what_it_writes_it_reads_back() {
    for standing in [
        Override {
            profile: "work".to_string(),
            until: None,
        },
        Override {
            profile: "work".to_string(),
            until: Some(42),
        },
    ] {
        assert_eq!(parse_override(&format_override(&standing)), Some(standing));
    }
}
