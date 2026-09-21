use super::excerpt;
use pns_domain::recap::activity::{Project, Session};

fn session(title: &str, path: &str) -> Session {
    Session {
        session: "s1".to_string(),
        title: title.to_string(),
        harness: "claude".to_string(),
        branch: String::new(),
        pane: String::new(),
        workspace: String::new(),
        model: String::new(),
        duration_secs: 0,
        last_state: String::new(),
        transcript_path: path.to_string(),
        events: Vec::new(),
    }
}

/// THE ASSISTANT'S OWN TURNS AND NOTHING ELSE, which is the whole privacy
/// rule: a user prompt carries paths and whatever was pasted into a session.
#[test]
fn only_the_assistant_turns_cross_and_both_ceilings_are_honoured() {
    let directory = std::env::temp_dir().join(format!("pns-transcripts-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"user\",\"message\":{\"content\":\"my secret token\"}}\n\
         {\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"I fixed the parser\"}]}}\n\
         {\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Bash\"}]}}\n",
    )
    .unwrap();
    let projects = vec![Project {
        project: "dotfiles".to_string(),
        sessions: vec![session("the parser", &path.display().to_string())],
    }];
    let appended = excerpt(&projects, 8192, 65536);
    assert!(appended.contains("I fixed the parser"), "{appended}");
    assert!(!appended.contains("my secret token"), "{appended}");
    assert!(!appended.contains("Bash"), "{appended}");
    // THE PER-SESSION CEILING CUTS THE EXCERPT rather than the session count,
    // so one long session cannot spend the whole window's budget.
    assert!(excerpt(&projects, 8, 65536).len() < appended.len());
    // AND A TOTAL OF NOTHING APPENDS NOTHING AT ALL.
    assert_eq!(excerpt(&projects, 8192, 0), "");
    // A SESSION WITH NO TRANSCRIPT IS SKIPPED, not an error: Codex sends one
    // and a harness outside pns's hooks sends none.
    let none = vec![Project {
        project: "dotfiles".to_string(),
        sessions: vec![session("no transcript", "")],
    }];
    assert_eq!(excerpt(&none, 8192, 65536), "");
    std::fs::remove_dir_all(&directory).ok();
}
