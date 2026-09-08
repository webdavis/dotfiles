use super::*;
#[test]
fn every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel() {
    // A LINE FROM THIS FUNCTION IS RENDERED SOMEWHERE THAT OBEYS IT: a
    // terminal banner, a herdr pane, a Discord post. C0 is not text, and
    // the whitespace this already flattens is the only part of it that
    // was ever handled, so ESC, BEL and NUL rode through verbatim.
    //
    // THE MOTIVATING FEEDER IS PROVIDER-CONTROLLED. `error` is whatever the
    // API said, and an escape sequence in it is the one string here nobody
    // on this machine wrote.
    // EVERY CODEPOINT IN THE SET, not a representative of each run. The
    // scrub is written as one category test, so the only way it can be
    // wrong is per codepoint, and a matrix of samples cannot see a single
    // exemption: measured, a `flattened` that let U+0002 through passed
    // every test in this crate while leaking that byte to a banner. The set
    // is `char::is_control` itself, which is exactly Cc: C0, DEL and C1.
    for codepoint in (0x00..=0x1f_u32).chain([0x7f]).chain(0x80..=0x9f) {
        let control = char::from_u32(codepoint).expect("a Cc codepoint");
        assert!(control.is_control(), "U+{codepoint:04X} is the Cc set");
        assert_eq!(
            one_line(&serde_json::Value::String(format!("a{control}b"))),
            "a b",
            "U+{codepoint:04X} reached a channel verbatim"
        );
    }

    // AND THE SEQUENCES THOSE BYTES ARRIVE IN, which the loop above cannot
    // state: a scrub that removed the escape and left `[31m` or an OSC
    // title behind would still pass every assertion up there, and what
    // reaches the operator is the whole sequence rather than one byte.
    for (raw, scrubbed, class) in [
        ("a\u{1b}[31mb", "a [31mb", "a colour sequence"),
        (
            "a\u{1b}]0;title\u{7}b",
            "a ]0;title b",
            "an OSC title sequence",
        ),
        // AND THE WHITESPACE IT ALREADY FLATTENED still flattens the same
        // way, which is what makes this a widening rather than a rewrite.
        ("a\nb\tc  d", "a b c d", "the whitespace it already handled"),
    ] {
        assert_eq!(
            one_line(&serde_json::Value::String(raw.to_string())),
            scrubbed,
            "{class} reached a channel verbatim"
        );
    }

    // ORDINARY TEXT IS UNTOUCHED, which is the control the sweep needs:
    // scrubbing by codepoint RANGE rather than by category would take
    // multibyte characters with it, and an operator's own prose is full of
    // them.
    for kept in ["café", "日本語", "→ ✓ ×", "naïve résumé ½ ±"] {
        assert_eq!(
            one_line(&serde_json::Value::String(kept.to_string())),
            kept,
            "text that is not a control byte must pass through"
        );
    }

    // EVERY SHAPE THE FUNCTION WALKS, because a scrub on the string arm
    // alone leaves the same byte reaching a channel one nesting level down,
    // and an object's KEY is provider-controlled exactly like its value.
    assert_eq!(
        one_line(&serde_json::json!(["a\u{1b}b", "c"])),
        "a b c",
        "an array member is scrubbed like a bare string"
    );
    assert_eq!(
        one_line(&serde_json::json!({"k\u{7}k": "v\u{1b}v"})),
        "k k=v v",
        "an object's key is scrubbed beside its value"
    );
}

#[test]
fn every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone() {
    // A CARD IS COMPOSED FROM FOUR PAYLOAD STRINGS and the scrub reached
    // one of them. `tool_input` went through `one_line` while the
    // `tool_name` formatted in front of it on the same line did not, and
    // `message` and `detail`, which are the first two of the chain and the
    // ones the common harnesses actually send, went through nothing at all.
    // The rule stated at `flattened` (a line from here is rendered
    // somewhere that OBEYS it) holds for every string on the card or it
    // holds for none of them: the same ESC reaches the same banner by
    // whichever road is left open.

    // ALL THREE IN ONE ASSERT, so a run names every field still riding
    // through rather than the first one only.
    //
    // `tool_name` is remote text: a connected Model Context Protocol server
    // names its own tools, and a Codex permission payload carries neither
    // `message` nor `detail`, so that name IS the whole card, shown at the
    // moment the operator is being asked to decide. `message` and `detail`
    // are the first two of the chain and carry the NEWLINE half of the
    // guarantee too, which is older than the control scrub: a second line
    // in either breaks the single rendered line every channel expects.
    let cards = [
        "{\"tool_name\":\"Bash\\u001b[2J\\u0007\",\"tool_input\":{\"c\":\"ls\"}}",
        "{\"message\":\"plan\\u001b[2J\\u0007 ready\\nsecond line\"}",
        "{\"detail\":\"a\\u0000b\\nc\"}",
    ]
    .map(|payload| parse_payload(payload).message);
    assert_eq!(
        cards,
        ["Bash [2J: c=ls", "plan [2J ready second line", "a b c"]
    );

    // AND A STRING THAT IS NOTHING BUT CONTROL BYTES SAYS NOTHING, so the
    // chain moves on to the next thing that was actually stated rather than
    // carding a blank where a message appeared to be.
    assert_eq!(
        parse_payload("{\"message\":\"\\u0007\",\"detail\":\"the real one\"}").message,
        "the real one"
    );
}

#[test]
fn a_provider_error_carrying_an_escape_sequence_cannot_dress_up_a_card() {
    // THROUGH THE PAYLOAD, so this pins the feeder and not only the
    // helper: the error field is the one string on this path that a remote
    // provider writes end to end.
    let payload = parse_payload("{\"error\":\"API Error: 500\\u001b[2J\\u0007 cleared\"}");
    assert_eq!(payload.message, "API Error: 500 [2J cleared");
}

#[test]
fn an_error_is_kept_to_one_line_and_cut_from_the_head_like_a_tool_request() {
    // A provider error arrives with a stack trace behind it often enough
    // that the raw string is a wall. A newline in it would break the
    // single rendered line every channel expects, and the cut keeps the
    // HEAD because an API error states its kind first.
    let payload = parse_payload(r#"{"error":"API Error: 500\n  at fetch\n  at main"}"#);
    assert_eq!(payload.message, "API Error: 500 at fetch at main");

    let long = "x".repeat(5_000);
    let payload = parse_payload(&format!(r#"{{"error":"API Error: {long}"}}"#));
    assert!(payload.message.starts_with("API Error: xxx"));
    // THE CAP ITSELF, spelled out rather than read back off the constant
    // the cut uses: an assertion phrased against `TOOL_REQUEST_MAX_CHARS`
    // agrees with whatever that constant is moved to, which is the one
    // thing this is here to catch.
    assert_eq!(
        payload.message.chars().count(),
        320,
        "an error is cut at the shared cap, not merely somewhere under it"
    );
}
