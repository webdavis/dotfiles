use super::*;

#[test]
fn include_patterns_unescape_twice_and_distinguish_literal_brackets() {
    for (raw, literal, has_glob) in [
        (
            b"pay\\load.conf".as_slice(),
            b"payload.conf".as_slice(),
            false,
        ),
        (b"pay\\\\load.conf", b"pay\\load.conf", false),
        (b"trailing\\", b"trailing\\", false),
        (b"z[]load", b"z[]load", false),
        (b"z[]]load", b"z[]]load", true),
        (b"z[ab\\]load", b"z[ab]load", false),
        (b"z[ab\\]c]load", b"z[ab]c]load", true),
        (b"[!^a].conf", b"[!^a].conf", true),
        (b"[\\^a].conf", b"[^a].conf", true),
        (b"z\\*load?.conf", b"z*load?.conf", true),
        (b"pa\\y[ab]load.conf", b"pay[ab]load.conf", true),
    ] {
        let values = analyze_include(b"/config", &[raw.to_vec()]).unwrap();
        assert_eq!(
            values,
            [IncludePattern {
                pattern: [b"/config/", raw].concat(),
                literal: [b"/config/", literal].concat(),
                has_glob
            }]
        );
    }
}

#[test]
fn missing_paths_and_divergent_caret_brackets_are_refused() {
    assert_eq!(
        analyze_include(b"/config", &[]),
        Err(IncludeRefusal::MissingPath)
    );
    assert_eq!(
        analyze_include(b"/config", &[b"[^a].conf".to_vec()]),
        Err(IncludeRefusal::CaretBracket(b"/config/[^a].conf".to_vec()))
    );
}

#[test]
fn raw_absolute_test_precedes_unescaping_and_argument_order_is_preserved() {
    let patterns = analyze_include(
        b"/config",
        &[
            b"\\/etc/ssh/x.conf".to_vec(),
            b"/absolute".to_vec(),
            b"b.conf".to_vec(),
            b"a.conf".to_vec(),
        ],
    )
    .unwrap();
    let paths: Vec<_> = patterns.iter().map(|p| p.literal.as_slice()).collect();
    assert_eq!(
        paths,
        [
            b"/config//etc/ssh/x.conf".as_slice(),
            b"/absolute",
            b"/config/b.conf",
            b"/config/a.conf"
        ]
    );
}
