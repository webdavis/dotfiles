use super::*;
#[test]
fn the_reply_is_the_assistant_text_of_the_last_turn_only() {
    let transcript = r#"{"type":"user","message":{"content":"first ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"an older answer"}]}}
{"type":"user","message":{"content":"second ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"the newest answer"}]}}"#;
    assert_eq!(transcript_reply(transcript), "the newest answer");
}

#[test]
fn several_text_blocks_in_one_turn_join_the_way_the_harness_renders_them() {
    let transcript = r#"{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"one"},{"type":"text","text":"two"}]}}"#;
    assert_eq!(transcript_reply(transcript), "one\n\ntwo");
}

#[test]
fn a_first_line_cut_in_half_by_the_tail_is_skipped_not_fatal() {
    // The reader takes the last few megabytes, so the first line is
    // routinely half an object. Refusing the whole transcript over it
    // would lose the reply on every long session.
    let transcript = r#"ge":{"content":[{"type":"text","text":"cut"}]}}
{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"kept"}]}}"#;
    assert_eq!(transcript_reply(transcript), "kept");
}

#[test]
fn a_transcript_with_no_readable_turn_yields_nothing_rather_than_guessing() {
    assert_eq!(transcript_reply(""), "");
    assert_eq!(transcript_reply("not json at all"), "");
    assert_eq!(
        transcript_reply(r#"{"type":"assistant","message":{"content":[]}}"#),
        ""
    );
}

#[test]
fn tool_blocks_are_not_the_reply() {
    let transcript = r#"{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"},{"type":"text","text":"said"}]}}"#;
    assert_eq!(transcript_reply(transcript), "said");
}

#[test]
fn the_name_the_operator_gave_the_session_wins_over_the_generated_one() {
    // THE ORDER THE RECAP DESIGN STATES, and the line shapes a real Claude
    // Code transcript carries: `custom-title` is the rename, `ai-title` is
    // the harness's own, and the newest of each is the one that counts.
    let transcript = r#"{"type":"ai-title","aiTitle":"generated words"}
{"type":"assistant","message":{"model":"claude-opus-5","content":[]}}
{"type":"custom-title","customTitle":"the name I gave it"}"#;
    let facts = session_facts(transcript);
    assert_eq!(facts.custom_title, "the name I gave it");
    assert_eq!(facts.ai_title, "generated words");
    assert_eq!(facts.model, "claude-opus-5");
}

#[test]
fn a_transcript_naming_no_title_says_so_rather_than_guessing_one() {
    // A CODEX ROLLOUT, whose lines carry neither title field, so the caller
    // falls back to what the session row already holds.
    let transcript = r#"{"type":"session_meta","payload":{"session_id":"s1"}}
{"type":"response_item","payload":{"role":"user"}}"#;
    assert_eq!(session_facts(transcript), SessionFacts::default());
}
