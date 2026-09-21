use super::*;

#[test]
fn the_summarizer_is_an_argument_list_the_operator_states_word_by_word() {
    // ARGV, NEVER A SHELL STRING. Nothing here is interpreted, so there is
    // no quoting rule to get wrong and no injection surface at all; a
    // different backend is a different array and a Linux machine writes its
    // own, which is the whole of the configurable-backend mandate.
    let config = parse_config(
        "[recap]\nsummarizer = [\"ollama\", \"run\", \"qwen3.5:4b\", \"--think=false\"]\n",
    )
    .unwrap();
    assert_eq!(
        config.recap.summarizer.as_deref(),
        Some(
            ["ollama", "run", "qwen3.5:4b", "--think=false"]
                .map(String::from)
                .as_slice()
        ),
        "the words the operator wrote, in order"
    );
    assert_eq!(
        parse_config("[recap]\npost_window_recap = true\n")
            .unwrap()
            .recap
            .summarizer,
        None,
        "UNSET IS THE WORKING SETTING: no summarizer is the plain lists"
    );
}

#[test]
fn a_summarizer_that_is_not_a_list_of_words_is_refused_naming_the_key() {
    // THE FOUR SHAPES A HAND WRITES BY MISTAKE: the shell string this key
    // deliberately is not, an array with something that is not a word in
    // it, an empty array that names no command at all, and an array whose
    // FIRST WORD is empty, which names no command either and used to parse.
    // `Command::new("")` fails to spawn, and the operator then reads a
    // summarizer that is not answering rather than the table they have to
    // fix, which is the exact outcome the empty-array refusal exists to
    // prevent. Each is refused by name rather than leaving the summarizer
    // silently unset, which is the difference between a recap that says it
    // fell back and one the operator believes is summarized.
    for (stated, expected) in [
        ("\"ollama run qwen3.5:4b\"", "not a list"),
        ("[\"ollama\", 3]", "not a list"),
        ("[]", "names no command"),
        ("[\"\"]", "names no command"),
        ("[\"\", \"run\"]", "names no command"),
    ] {
        let err = parse_config(&format!("[recap]\nsummarizer = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("summarizer"),
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
fn the_summarizers_deadline_is_a_duration_with_a_generous_default() {
    // FOUR MINUTES, and what it covers is GENERATION. MEASURED on one
    // machine: a whole three-call episode took about 114.6 seconds, nearly
    // all of it tokens arriving at about eleven a second, while the cold
    // model load cost about 5.5 seconds and was paid once. Nobody is
    // waiting: the caller is a process the event path never joined.
    assert_eq!(
        parse_config("[recap]\npost_window_recap = true\n")
            .unwrap()
            .recap
            .summarizer_deadline,
        Duration::from_secs(240),
        "the default is generous against a measured episode"
    );
    // A DURATION STRING, the same one a flag and every other key takes, and
    // finer than a second so a test can prove expiry without waiting.
    assert_eq!(
        parse_config("[recap]\nsummarizer_deadline = \"3s\"\n")
            .unwrap()
            .recap
            .summarizer_deadline,
        Duration::from_secs(3)
    );
    assert_eq!(
        parse_config("[recap]\nsummarizer_deadline = \"300ms\"\n")
            .unwrap()
            .recap
            .summarizer_deadline,
        Duration::from_millis(300)
    );
    // ZERO IS ACCEPTED AND IS NOT A TRAP, unlike `minimum_events`'s zero: a
    // deadline of nothing cannot be met, so the recap falls to the plain
    // lists and says it did.
    assert_eq!(
        parse_config("[recap]\nsummarizer_deadline = \"0s\"\n")
            .unwrap()
            .recap
            .summarizer_deadline,
        Duration::ZERO
    );
    // AND IT HAS A TOP END, refused by name for `minimum_events`'s own reason.
    // An hour is already far past the default, and past it the two failures
    // are real: nothing supervises the detached recap child, so a wedged
    // backend holds one child and one backend process for as long as the
    // number says, and a duration past the ceiling PANICS the child at
    // `Instant::now() + deadline` (MEASURED: "overflow when adding duration
    // to instant"). That panic lands in a process whose stderr is /dev/null
    // and whose exit code nobody reads, so the recap vanishes with no rung
    // of the ladder taken, after the card has already said it is coming.
    assert_eq!(
        parse_config("[recap]\nsummarizer_deadline = \"3600s\"\n")
            .unwrap()
            .recap
            .summarizer_deadline,
        Duration::from_secs(3600),
        "an hour is inside the ceiling"
    );
    for stated in ["\"soon\"", "240", "\"3601s\"", "\"2h\""] {
        let err = parse_config(&format!("[recap]\nsummarizer_deadline = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("summarizer_deadline"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}

#[test]
fn the_retired_seconds_spelling_of_the_deadline_is_refused_by_name() {
    // THE OLD KEY IS NOT SILENTLY IGNORED. It counted bare seconds where
    // every flag and key beside it took a duration string, so a file still
    // carrying it would otherwise run the shipped default while the
    // operator read their own number off the file.
    let err = parse_config("[recap]\nsummarizer_deadline_secs = 240\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("summarizer_deadline_secs"),
                "the retired key is named: {message}"
            );
            assert!(
                message.contains("summarizer_deadline"),
                "and the spelling that replaced it is listed: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}
