use super::*;

fn projected(raw: &[u8], label: Option<&str>) {
    assert_eq!(
        source_line(raw.to_vec()),
        SourceLine::Object {
            label: label.map(str::to_owned),
            raw: raw.to_vec()
        }
    );
}
#[test]
fn an_inert_nan_infinity_or_leading_zero_keeps_its_original_bytes() {
    for number in ["NaN", "Infinity", "01"] {
        let raw = format!("{{\"label\":\"other\",\"unused\":{number}}}");
        projected(raw.as_bytes(), Some("other"));
    }
}
#[test]
fn label_numbers_keep_the_measured_decimal_spelling_and_nan_is_a_nonempty_null_label() {
    for (number, label) in [
        ("12", "12"),
        ("12.0", "12.0"),
        ("01", "1"),
        ("NaN", "null"),
        ("1e2", "1E+2"),
    ] {
        projected(format!("{{\"label\":{number}}}").as_bytes(), Some(label));
    }
}
#[test]
fn duplicate_labels_use_the_last_value_without_rewriting_the_source_object() {
    projected(br#"{"label":"first","label":"last"}"#, Some("last"));
}
#[test]
fn false_missing_and_structured_labels_cannot_match_a_valid_writer_label() {
    for raw in [
        br#"{"label":false}"#.as_slice(),
        br#"{}"#,
        br#"{"label":null}"#,
        br#"{"label":[]}"#,
        br#"{"label":{}}"#,
    ] {
        projected(raw, None);
    }
}
#[test]
fn low_surrogate_and_invalid_utf8_projection_preserve_the_original_source_bytes() {
    projected(br#"{"label":"\udc00"}"#, Some("\u{fffd}"));
    projected(b"{\"label\":\"\xff\"}", Some("\u{fffd}"));
}
#[test]
fn unmatched_high_surrogates_are_refused_even_in_an_inert_field() {
    for raw in [
        br#"{"label":"other","unused":"\ud800"}"#.as_slice(),
        br#"{"label":"other","unused":"\ud800\ud000"}"#,
    ] {
        assert_eq!(source_line(raw.to_vec()), SourceLine::Invalid);
    }
    projected(br#"{"label":"\ud83d\ude00"}"#, Some("😀"));
}
#[test]
fn strings_that_look_like_numbers_or_escapes_do_not_become_parser_tokens() {
    projected(
        br#"{"label":"NaN","unused":"\\ud800 01 Infinity"}"#,
        Some("NaN"),
    );
}
#[test]
fn source_grammar_requires_one_complete_object_and_preserves_only_true_comments_and_blanks() {
    for raw in [
        b"[]".as_slice(),
        b"{}{}",
        b"{bad",
        b" ",
        b" #comment",
        b"{\"label\":NaN1}",
    ] {
        assert_eq!(source_line(raw.to_vec()), SourceLine::Invalid);
    }
    for raw in [b"".as_slice(), b"# comment"] {
        assert_eq!(
            source_line(raw.to_vec()),
            SourceLine::Preserved(raw.to_vec())
        );
    }
}
#[test]
fn labels_discard_nul_and_trailing_newlines_as_bash_command_substitution_does() {
    projected(br#"{"label":"my\u0000.alpha\n\n"}"#, Some("my.alpha"));
}
#[test]
fn deeply_nested_inert_fields_do_not_use_a_recursive_value_tree() {
    let raw = format!(
        "{{\"label\":\"my.alpha\",\"unused\":{}0{}}}",
        "[".repeat(512),
        "]".repeat(512)
    );
    projected(raw.as_bytes(), Some("my.alpha"));
}

#[test]
fn source_projection_preserves_the_existing_container_stack_boundary() {
    for (open, close, depth, accepted) in [
        ("[", "]", 9998, true),
        ("[", "]", 9999, false),
        ("{\"x\":", "}", 4999, true),
        ("{\"x\":", "}", 5000, false),
    ] {
        let raw = format!(
            "{{\"unused\":{}0{}}}",
            open.repeat(depth),
            close.repeat(depth)
        );
        assert_eq!(
            !matches!(source_line(raw.into_bytes()), SourceLine::Invalid),
            accepted,
            "{open} depth {depth}"
        );
    }
    for (depth, accepted) in [(9997, true), (9998, false)] {
        let raw = format!(
            "{{\"unused\":{}{{\"x\":0}}{}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert_eq!(
            !matches!(source_line(raw.into_bytes()), SourceLine::Invalid),
            accepted,
            "object at array depth {depth}"
        );
    }
}
