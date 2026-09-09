use super::*;

fn failures(text: &str) -> Failures {
    parse_config(text).expect("this config parses").failures
}

/// A file with no table gets the page on, at the documented port. Both are
/// defaulted keys rather than an opt-in feature, which is what the shipped
/// template says by writing them uncommented.
#[test]
fn a_file_with_no_table_serves_the_page_at_the_documented_port() {
    let defaults = failures("");
    assert!(defaults.serve);
    assert_eq!(defaults.port, 8646);
}

/// `false` is the fallback for a phone that cannot reach the page, and it must
/// leave the port alone: the operator turning the page off is not also
/// forgetting which port they had chosen.
#[test]
fn serving_can_be_switched_off_without_disturbing_the_port() {
    let off = failures("[failures]\nserve = false\nport = 9000\n");
    assert!(!off.serve);
    assert_eq!(off.port, 9000);
}

/// A privileged port is a config that cannot do what it says: the daemon runs
/// as the operator and could never bind it.
#[test]
fn a_privileged_port_is_refused_by_name() {
    let error = refusal("[failures]\nport = 80\n");
    assert!(error.contains("`port`"), "{error}");
    assert!(error.contains("80"), "{error}");
}

/// A number no port can be is refused for the same reason, at the other end.
#[test]
fn a_port_above_the_range_is_refused_by_name() {
    let error = refusal("[failures]\nport = 70000\n");
    assert!(error.contains("70000"), "{error}");
}

/// A key this table does not serve is a typo the operator believes they set,
/// so it is named rather than ignored.
#[test]
fn an_unknown_key_is_refused_by_name() {
    let error = refusal("[failures]\nserved = true\n");
    assert!(error.contains("served"), "{error}");
    assert!(error.contains("failures"), "{error}");
}

/// A value of the wrong shape says which key and what it got, because "invalid
/// config" without a noun is a hunt.
#[test]
fn a_value_of_the_wrong_shape_names_the_key_and_its_type() {
    let error = refusal("[failures]\nserve = \"yes\"\n");
    assert!(error.contains("`serve`"), "{error}");
    assert!(error.contains("string"), "{error}");
}

fn refusal(text: &str) -> String {
    match parse_config(text) {
        Err(ConfigError::Invalid(message)) => message,
        other => panic!("expected a named refusal, got {other:?}"),
    }
}
