use super::fixtures::*;

#[test]
fn an_unknown_type_with_control_bytes_is_escaped_like_every_other_spelled_value() {
    let line = setup_report(&SetupFailure::UnknownType("a\u{1b}[31mz".to_string()));
    assert!(
        !line.contains('\u{1b}'),
        "raw ESC must not reach stdout: {line}"
    );
    assert!(
        line.contains("\\u{1b}"),
        "the escaped form is shown: {line}"
    );
}
