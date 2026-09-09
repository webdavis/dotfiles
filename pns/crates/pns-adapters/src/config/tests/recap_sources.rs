use super::*;

#[test]
fn the_two_external_sources_are_named_by_the_operator_or_not_read_at_all() {
    // NEITHER SECTION HAS A SOURCE pns can find on its own: merged pull
    // requests live in a repository nothing here knows the name of, and the
    // review notes live wherever this operator's own pipeline puts them.
    // Both are therefore keys, and an absent key is the working setting.
    let config = parse_config(
        "[recap]\nrepos = [\"webdavis/dotfiles\"]\n\
             review_notes = \"~/.claude/pipeline/slices/checklist-*.md\"\n",
    )
    .unwrap();
    assert_eq!(config.recap.repos, ["webdavis/dotfiles".to_string()]);
    assert_eq!(
        config.recap.review_notes.as_deref(),
        Some("~/.claude/pipeline/slices/checklist-*.md")
    );
    let unconfigured = parse_config("[recap]\ndigest = true\n").unwrap().recap;
    assert!(
        unconfigured.repos.is_empty(),
        "UNSET IS THE WORKING SETTING: no repo is no `gh` at all"
    );
    assert_eq!(
        unconfigured.review_notes, None,
        "and no glob is no directory read at all"
    );
}

#[test]
fn a_repos_value_that_is_not_repository_names_is_refused_naming_the_key() {
    // THE SAME FOUR SHAPES `summarizer` REFUSES, for the same reason: a
    // list this layer reads itself is a list it can judge, and a repo name
    // it silently dropped would read to the operator as a night with no
    // merges in it rather than as a table they have to fix.
    for (stated, expected) in [
        ("\"webdavis/dotfiles\"", "not a list"),
        ("[\"webdavis/dotfiles\", 3]", "not a list"),
        ("[]", "names no repository"),
        ("[\"\"]", "names no repository"),
    ] {
        let err = parse_config(&format!("[recap]\nrepos = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("repos"),
                    "the offender is named for {stated}: {message}"
                );
                assert!(
                    message.contains(expected),
                    "the refusal says what is wrong for {stated}: {message}"
                );
            }
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}

#[test]
fn a_review_notes_glob_that_names_no_readable_file_is_refused_naming_the_key() {
    // THE GLOB IS THE WHOLE PERMISSION. It is the only thing that decides
    // which files pns opens, so a shape it cannot resolve exactly is
    // refused rather than resolved generously: a RELATIVE path would
    // resolve against whatever directory the return event happened to be
    // in, and a `*` in a DIRECTORY would make the set of directories pns
    // reads a search rather than a statement.
    for (stated, expected) in [
        ("3", "not a path"),
        ("\"\"", "names no file"),
        ("\"slices/checklist-*.md\"", "absolute"),
        ("\"~/.claude/*/checklist-*.md\"", "file name may hold a"),
        ("\"~/.claude/checklist-*-*.md\"", "only one"),
    ] {
        let err = parse_config(&format!("[recap]\nreview_notes = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("review_notes"),
                    "the offender is named for {stated}: {message}"
                );
                assert!(
                    message.contains(expected),
                    "the refusal says what is wrong for {stated}: {message}"
                );
            }
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}
