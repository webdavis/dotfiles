use super::*;

#[test]
fn run_completes_a_jump_over_the_socket_without_touching_the_cli() {
    let list =
        r#"{"id":"req_1","result":{"workspaces":[{"workspace_id":"w11","label":"dotfiles"}]}}"#;
    let ok = r#"{"id":"req_2","result":{"type":"ok"}}"#;
    let server = crate::socket_server::SocketServer::start(vec![
        crate::socket_server::Reply::Line(list.to_string()),
        crate::socket_server::Reply::Line(ok.to_string()),
    ]);
    let path = &server.path;

    // The CLI binary does not exist, so a fallback would fail the assertion.
    let outcome = run(
        Some(path.to_string_lossy().to_string()),
        Some("/nonexistent/herdr-binary".to_string()),
        "dotfiles",
        "/tmp",
    );

    assert_eq!(outcome, Ok(Jump::Focused("w11".to_string())));
}
#[test]
fn run_falls_back_to_the_cli_when_the_socket_path_is_absent() {
    // Both paths are dead, so the surviving error proves which one answered
    // last: the CLI failure, carrying the socket failure behind it.
    let failure = run(
        None,
        Some("/nonexistent/herdr-binary".to_string()),
        "dotfiles",
        "/tmp",
    );
    let rendered = match failure {
        Err(error) => error.to_string(),
        Ok(outcome) => panic!("expected both paths to fail, got {outcome:?}"),
    };
    assert!(rendered.contains("/nonexistent/herdr-binary"), "{rendered}");
    assert!(
        rendered.contains("HERDR_SOCKET_PATH is unset"),
        "{rendered}"
    );
    // Exactly one prefix per failure, never the wrapper's stacked on top of
    // the wrapped one's. Two failures are named, so two prefixes.
    assert_eq!(
        rendered.matches("herdr unreachable").count(),
        2,
        "{rendered}"
    );
}
#[test]
fn run_falls_back_to_the_cli_when_the_socket_cannot_be_opened() {
    let absent = std::path::PathBuf::from("/nonexistent/hwj-run-no-socket");
    let failure = run(
        Some(absent.to_string_lossy().to_string()),
        Some("/nonexistent/herdr-binary".to_string()),
        "dotfiles",
        "/tmp",
    );
    let rendered = match failure {
        Err(error) => error.to_string(),
        Ok(outcome) => panic!("expected both paths to fail, got {outcome:?}"),
    };
    assert!(rendered.contains("hwj-run-no-socket"), "{rendered}");
}
#[test]
fn run_lists_and_focuses_through_the_cli_when_the_socket_is_absent() {
    let fake = crate::cli_command::RecordedCli::answering(
        r#"{"result":{"workspaces":[{"workspace_id":"w11","label":"dotfiles"}]}}"#,
        "",
    );
    assert_eq!(
        run(None, Some(fake.binary.clone()), "dotfiles", "/tmp"),
        Ok(Jump::Focused("w11".into()))
    );
    assert_eq!(
        fake.calls(),
        vec![vec!["workspace", "list"], vec!["workspace", "focus", "w11"]]
    );
}
#[test]
fn a_blank_socket_acknowledgment_retries_the_jump_through_the_actual_cli() {
    let list = r#"{"result":{"workspaces":[{"workspace_id":"w11","label":"dotfiles"}]}}"#;
    let server = crate::socket_server::SocketServer::start(vec![
        crate::socket_server::Reply::Line(list.into()),
        crate::socket_server::Reply::Line(String::new()),
    ]);
    let fake = crate::cli_command::RecordedCli::answering(list, "");
    assert_eq!(
        run(
            Some(server.path.to_string_lossy().into_owned()),
            Some(fake.binary.clone()),
            "dotfiles",
            "/tmp"
        ),
        Ok(Jump::Focused("w11".into()))
    );
    assert_eq!(server.seen()["method"], "workspace.list");
    assert_eq!(server.seen()["method"], "workspace.focus");
    assert_eq!(
        fake.calls(),
        vec![vec!["workspace", "list"], vec!["workspace", "focus", "w11"]]
    );
}
