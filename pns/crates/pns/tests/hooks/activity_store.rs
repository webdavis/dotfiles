use super::*;

/// The activity row a hook wrote, as the recap will read it.
fn rows(sandbox: &Sandbox) -> Vec<(String, String, String, String, String, String)> {
    stored_records::database(sandbox)
        .prepare(
            "SELECT state, session, session_title, pane, workspace, model
               FROM activity_events ORDER BY seq",
        )
        .expect("the activity table")
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .expect("read the rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("activity rows")
}

#[test]
fn a_prompt_writes_one_row_naming_the_session_the_transcript_names() {
    let sandbox = Sandbox::new("hook-activity-row");
    let transcript = sandbox.path("transcript.jsonl");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(
        &transcript,
        "{\"type\":\"ai-title\",\"aiTitle\":\"generated words\"}\n\
         {\"type\":\"assistant\",\"message\":{\"model\":\"claude-opus-5\",\"content\":[]}}\n\
         {\"type\":\"custom-title\",\"customTitle\":\"the name I gave it\"}\n",
    )
    .expect("a transcript");
    let mut command = with_state_dir(&sandbox);
    command.env("HERDR_PANE_ID", "w7:p2");
    command.env("HERDR_WORKSPACE_ID", "w7");
    let output = hook_with(
        command,
        &sandbox,
        "prompt",
        &format!(
            r#"{{"session_id":"s1","prompt":"what the operator typed","transcript_path":"{}"}}"#,
            transcript.display()
        ),
    );
    assert!(output.status.success());
    assert_eq!(
        rows(&sandbox),
        vec![(
            "prompt".to_string(),
            "s1".to_string(),
            // THE OPERATOR'S OWN NAME WINS over the generated one and over
            // the first prompt, which is the order the recap design states.
            "the name I gave it".to_string(),
            "w7:p2".to_string(),
            "w7".to_string(),
            "claude-opus-5".to_string(),
        )],
        "one hook event is one row"
    );
}

#[test]
fn a_resolved_row_carries_the_sessions_stored_title_and_reads_no_transcript() {
    let sandbox = Sandbox::new("hook-activity-resolved");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1","prompt":"what the operator typed"}"#,
    );
    // A TRANSCRIPT NAMING A DIFFERENT TITLE, so a row that read it would
    // disagree with the one the prompt already stored: proof `resolved`
    // never opens the file at all, not just that it reports the same answer.
    let transcript = sandbox.path("transcript.jsonl");
    std::fs::write(&transcript, "{\"type\":\"custom-title\",\"customTitle\":\"a transcript title resolved must never read\"}\n")
        .expect("a transcript");
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "resolved",
        &format!(
            r#"{{"session_id":"s1","transcript_path":"{}"}}"#,
            transcript.display()
        ),
    );
    assert!(output.status.success());
    let row = rows(&sandbox).pop().expect("the resolved row");
    assert_eq!(row.0, "resolved");
    assert_eq!(
        row.2, "what the operator typed",
        "the stored title, not the transcript's"
    );
}

#[test]
fn a_session_title_falls_back_to_the_prompt_when_the_transcript_names_none() {
    let sandbox = Sandbox::new("hook-activity-fallback");
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1","prompt":"what the operator typed"}"#,
    );
    assert!(output.status.success());
    let row = rows(&sandbox).pop().expect("one row");
    assert_eq!(row.2, "what the operator typed");
    assert_eq!(row.5, "", "no transcript states no model");
}
