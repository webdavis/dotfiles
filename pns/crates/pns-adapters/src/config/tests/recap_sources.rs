use super::*;

#[test]
fn every_list_section_is_named_by_the_operator_or_not_read_at_all() {
    // NO LIST SECTION HAS A SOURCE pns can find on its own: pull requests
    // live in repositories nothing here knows the name of, tasks live in
    // whichever task tool this operator uses, and the review notes live
    // wherever their own pipeline puts them. All are therefore keys, and an
    // absent key is the working setting.
    let config = parse_config(
        "[recap]\nreview_notes_glob = \"~/.claude/pipeline/slices/checklist-*.md\"\n\
         [recap.sources]\n\
         pull_requests = [\"gh\", \"pr\", \"list\", \"--search\", \"updated:>={since}\"]\n\
         tasks = [\"dam\", \"ls\"]\n",
    )
    .unwrap();
    assert_eq!(
        config.recap.sources.pull_requests.as_deref(),
        Some(
            ["gh", "pr", "list", "--search", "updated:>={since}"]
                .map(String::from)
                .as_slice()
        )
    );
    assert_eq!(
        config.recap.sources.tasks.as_deref(),
        Some(["dam", "ls"].map(String::from).as_slice())
    );
    assert_eq!(
        config.recap.review_notes_glob.as_deref(),
        Some("~/.claude/pipeline/slices/checklist-*.md")
    );
    let unconfigured = parse_config("[recap]\npost_window_recap = true\n")
        .unwrap()
        .recap;
    assert_eq!(
        unconfigured.sources,
        pns_domain::recap::Sources::default(),
        "UNSET IS THE WORKING SETTING: no command is no process at all"
    );
    assert_eq!(
        unconfigured.review_notes_glob, None,
        "and no glob is no directory read at all"
    );
}

#[test]
fn a_source_value_that_is_not_command_words_is_refused_naming_the_key() {
    // THE SAME FOUR SHAPES `summarizer` REFUSES, for the same reason: a
    // list this layer reads itself is a list it can judge, and a command it
    // silently dropped would read to the operator as a window with nothing
    // in it rather than as a table they have to fix.
    for (stated, expected) in [
        ("\"dam ls\"", "not a list"),
        ("[\"dam\", 3]", "not a list"),
        ("[]", "names no command"),
        ("[\"\"]", "names no command"),
    ] {
        let err = parse_config(&format!("[recap.sources]\ntasks = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("tasks"),
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
fn a_source_key_nothing_serves_is_refused_by_name() {
    let err = parse_config("[recap.sources]\nreviews = [\"ls\"]\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(message.contains("reviews"), "{message}"),
        other => panic!("expected Invalid, got {other:?}"),
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
        let err = parse_config(&format!("[recap]\nreview_notes_glob = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("review_notes_glob"),
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
