use super::*;
use pns_domain::lights::streak::WORKING;

/// herdr 0.8.2's own answer, captured live on 2026-09-01: three workspaces
/// carrying three of the four status words.
pub const HERDR_WORKSPACES: &str = r#"{"result":{"workspaces":[
  {"active_tab_id":"t1","agent_status":"working","focused":true,"workspace_id":"w1"},
  {"active_tab_id":"t4","agent_status":"idle","focused":false,"workspace_id":"w2"},
  {"active_tab_id":"t7","agent_status":"unknown","focused":false,"workspace_id":"w3"}
]}}"#;

/// The answer the suite's SHIPPED stub gives, which carries no
/// `agent_status` at all.
pub const NO_STATUS_FIELD: &str =
    r#"{"result":{"workspaces":[{"active_tab_id":"t1","focused":true,"workspace_id":"w1"}]}}"#;

#[test]
fn every_workspaces_agent_status_is_read_and_a_missing_one_is_not_working() {
    assert_eq!(
        workspace_agent_statuses(HERDR_WORKSPACES),
        vec![WORKING, "idle", "unknown"],
        "herdr's real answer, in its own order"
    );
    assert_eq!(
        workspace_agent_statuses(NO_STATUS_FIELD),
        vec![String::new()],
        "a workspace with no agent_status is a workspace this will not call working"
    );
    assert!(
        workspace_agent_statuses("not json").is_empty(),
        "an unreadable answer names no working workspace"
    );
}
