use super::*;
use pns_domain::recap::summarizer::Kind;

/// Every refusal this table makes is named, so a message that named only the
/// value would send an operator to the wrong heading.
fn refusal(stated: &str) -> String {
    match parse_config(stated).unwrap_err() {
        ConfigError::Invalid(message) => message,
        other => panic!("expected Invalid for {stated}, got {other:?}"),
    }
}

#[test]
fn the_summarizer_is_a_harness_named_by_word_and_custom_carries_the_words() {
    // A HARNESS IS NAMED, NEVER SPELLED OUT. pns owns `claude -p` and every
    // other known invocation, so a flag that moves is a pns release rather
    // than an edit in every operator's config.
    let config =
        parse_config("[recap.summarizer]\ntype = \"claude\"\nmodel = \"haiku\"\n").unwrap();
    assert_eq!(config.recap.summarizer.kind, Kind::Claude);
    assert_eq!(config.recap.summarizer.model, "haiku");
    assert!(config.recap.summarizer.configured());
    // ARGV, NEVER A SHELL STRING: nothing is interpreted, so there is no
    // quoting rule to get wrong and no injection surface at all.
    let custom = parse_config(
        "[recap.summarizer]\ntype = \"custom\"\ncommand = [\"my-model\", \"--quiet\"]\n",
    )
    .unwrap();
    assert_eq!(custom.recap.summarizer.command, ["my-model", "--quiet"]);
    // AND THE SHIPPED DEFAULT IS NO SUMMARIZER AT ALL, which is a working
    // setting and the common one: the recap writes no summary section.
    let bare = parse_config("[recap]\npost_window_recap = true\n").unwrap();
    assert_eq!(bare.recap.summarizer.kind, Kind::Custom);
    assert!(!bare.recap.summarizer.configured());
}

#[test]
fn a_table_that_contradicts_itself_is_refused_naming_the_two_keys() {
    // THE FIVE SHAPES A HAND WRITES BY MISTAKE, each refused by name rather
    // than resolved silently, which is the difference between a recap that
    // says it has no summary and one the operator believes is summarized.
    for (stated, expected) in [
        ("type = \"gpt\"\n", "no summarizer"),
        ("type = \"claude\"\ncommand = [\"claude\"]\n", "custom"),
        ("command = [\"\", \"run\"]\n", "empty word"),
        ("type = \"ollama\"\n", "no `model`"),
        (
            "type = \"claude\"\nprompt = \"say it\"\nprompt_file = \"/tmp/p\"\n",
            "only one may be stated",
        ),
    ] {
        let message = refusal(&format!("[recap.summarizer]\n{stated}"));
        assert!(
            message.contains("recap.summarizer"),
            "the table is named for {stated}: {message}"
        );
        assert!(
            message.contains(expected),
            "the refusal says what is wrong for {stated}: {message}"
        );
    }
    // AND AN UNKNOWN KEY IS REFUSED WITH THE ROSTER, as every other table's is.
    assert!(refusal("[recap.summarizer]\ntimeout = \"4m\"\n").contains("deadline"));
}

#[test]
fn the_summarizers_deadline_is_a_duration_with_a_generous_default() {
    // FOUR MINUTES, and what it covers is GENERATION. MEASURED on one
    // machine: a whole three-call episode took about 114.6 seconds, nearly
    // all of it tokens arriving at about eleven a second, while the cold
    // model load cost about 5.5 seconds and was paid once.
    assert_eq!(
        parse_config("[recap]\npost_window_recap = true\n")
            .unwrap()
            .recap
            .summarizer
            .deadline,
        Duration::from_secs(240),
        "the default is generous against a measured episode"
    );
    // A DURATION STRING, the same one a flag and every other key takes, and
    // finer than a second so a test can prove expiry without waiting.
    for (stated, expected) in [
        ("\"3s\"", Duration::from_secs(3)),
        ("\"300ms\"", Duration::from_millis(300)),
        ("\"3600s\"", Duration::from_secs(3600)),
    ] {
        assert_eq!(
            parse_config(&format!("[recap.summarizer]\ndeadline = {stated}\n"))
                .unwrap()
                .recap
                .summarizer
                .deadline,
            expected
        );
    }
    // AND IT HAS BOTH ENDS. Past an hour a wedged backend holds a child for
    // as long as the number says, and a duration past the type's own ceiling
    // PANICS at `Instant::now() + deadline` inside a process whose stderr is
    // /dev/null, so the recap vanishes after the card has said it is coming.
    // ZERO IS ACCEPTED AND IS NOT A TRAP: a deadline of nothing cannot be
    // met, so the summary is the one visible line and says so.
    assert_eq!(
        parse_config("[recap.summarizer]\ndeadline = \"0s\"\n")
            .unwrap()
            .recap
            .summarizer
            .deadline,
        Duration::ZERO
    );
    for stated in ["\"soon\"", "240", "\"3601s\"", "\"2h\""] {
        assert!(
            refusal(&format!("[recap.summarizer]\ndeadline = {stated}\n")).contains("deadline"),
            "the offender is named for {stated}"
        );
    }
}

#[test]
fn the_transcript_switch_and_its_two_ceilings_ship_at_the_designs_figures() {
    let shipped = parse_config("[recap]\npost_window_recap = true\n")
        .unwrap()
        .recap
        .summarizer;
    assert!(!shipped.transcripts, "transcripts are opt-in");
    assert_eq!(shipped.transcript_bytes_per_session, 8 * 1024);
    assert_eq!(shipped.transcript_bytes_total, 64 * 1024);
    let stated = parse_config(
        "[recap.summarizer]\ntranscripts = true\ntranscript_bytes_per_session = 100\n\
         transcript_bytes_total = 200\n",
    )
    .unwrap()
    .recap
    .summarizer;
    assert!(stated.transcripts);
    assert_eq!(stated.transcript_bytes_per_session, 100);
    assert_eq!(stated.transcript_bytes_total, 200);
    for stated in ["transcripts = \"yes\"", "transcript_bytes_total = -1"] {
        assert!(refusal(&format!("[recap.summarizer]\n{stated}\n")).contains("recap.summarizer"));
    }
}

#[test]
fn the_retired_spellings_of_the_summarizer_are_refused_by_name() {
    // NEITHER IS SILENTLY IGNORED. The argv key and the flat deadline both
    // moved under `[recap.summarizer]`, and a file still carrying one would
    // otherwise run the shipped default while the operator read their own
    // setting off the file.
    assert!(
        refusal("[recap]\nsummarizer = [\"ollama\", \"run\", \"m\"]\n")
            .contains("`recap.summarizer` is not a table"),
        "the argv spelling is refused as the table it became"
    );
    for retired in [
        "summarizer_deadline = \"3s\"",
        "summarizer_deadline_secs = 240",
    ] {
        let message = refusal(&format!("[recap]\n{retired}\n"));
        assert!(message.contains("summarizer"), "{message}");
    }
}

#[test]
fn pregenerate_names_windows_and_anything_else_is_refused() {
    assert!(
        parse_config("[recap]\npost_window_recap = true\n")
            .unwrap()
            .recap
            .pregenerate
            .is_empty(),
        "the gateway pregenerates nothing until a window is named"
    );
    assert_eq!(
        parse_config("[recap]\npregenerate = [\"morning\", \"week\"]\n")
            .unwrap()
            .recap
            .pregenerate,
        ["morning", "week"]
    );
    let message = refusal("[recap]\npregenerate = [\"mroning\"]\n");
    assert!(
        message.contains("mroning") && message.contains("morning"),
        "{message}"
    );
}
