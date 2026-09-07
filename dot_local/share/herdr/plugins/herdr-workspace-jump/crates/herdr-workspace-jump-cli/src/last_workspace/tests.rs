use super::*;
use std::path::PathBuf;

use herdr_workspace_jump_adapters::FileWorkspaceHistory;
use herdr_workspace_jump_domain::Mru;

use crate::cli_command::RecordedCli;
use crate::socket_server::{Reply, SocketServer};

fn fixture(answer: bool) -> (SocketServer, RecordedCli, FileWorkspaceHistory) {
    let list = r#"{"result":{"workspaces":[{"workspace_id":"wA"},{"workspace_id":"wB"}]}}"#;
    let fake = RecordedCli::answering(list, "");
    let state = PathBuf::from(&fake.binary).with_file_name("mru");
    crate::command::record_at(&state, r#"{"data":{"workspace_id":"wA"}}"#);
    crate::command::record_at(&state, r#"{"data":{"workspace_id":"wB"}}"#);
    let history = FileWorkspaceHistory::at(&state);
    let server = SocketServer::start(vec![
        Reply::Line(list.into()),
        Reply::RecordFocus { state, answer },
    ]);
    (server, fake, history)
}

#[test]
fn a_lost_focus_reply_retries_the_same_toggle_target_after_record_changes() {
    let (server, fake, mut history) = fixture(false);
    assert_eq!(
        history.read(),
        Mru {
            current: "wB".into(),
            previous: "wA".into()
        }
    );
    let result = run(
        Some(server.path.to_string_lossy().into()),
        Some(fake.binary.clone()),
        &mut history,
    );
    assert_eq!(
        history.read(),
        Mru {
            current: "wA".into(),
            previous: "wB".into()
        }
    );
    assert_eq!(server.seen()["method"], "workspace.list");
    assert_eq!(server.seen()["params"]["workspace_id"], "wA");
    assert_eq!(
        fake.calls(),
        vec![vec!["workspace", "list"], vec!["workspace", "focus", "wA"]]
    );
    assert_eq!(result, Ok(Bounce::Focus("wA".into())));
}

#[test]
fn an_acknowledged_toggle_records_focus_without_trying_the_cli() {
    let (server, _fake, mut history) = fixture(true);
    assert_eq!(
        run(
            Some(server.path.to_string_lossy().into()),
            Some("/nonexistent/herdr-binary".into()),
            &mut history
        ),
        Ok(Bounce::Focus("wA".into()))
    );
    assert_eq!(
        history.read(),
        Mru {
            current: "wA".into(),
            previous: "wB".into()
        }
    );
}
