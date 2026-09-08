use super::*;
use pns_application::DecisionOutcomes;
use pns_domain::{
    DecisionRequest, EnvironmentSnapshot, EventArgs, Overrides, Record, registry::Registry,
};
use std::time::{Duration, Instant};

fn begin(store: &SqliteStore, identity: &SubmissionIdentity) {
    let event = EventArgs::default();
    let overrides = Overrides::default();
    let decision = pns_domain::decide(
        &EnvironmentSnapshot::default(),
        &Registry::new().all(),
        &overrides,
        DecisionRequest {
            observation: false,
            silence_policy: pns_domain::SilencePolicy::Respect,
            local_only: false,
            remote_only: false,
            pane: "",
            now_secs: Some(7),
            long_running: false,
            mobile_watch_card: false,
        },
    );
    store
        .begin(
            identity,
            &Record {
                event: &event,
                decision: &decision,
                overrides: &overrides,
                legs: &[],
                nag: false,
                permission_mode: "private-policy",
                agent_id: "original-agent",
                tool_name: "original-tool",
            },
        )
        .unwrap();
}
fn line(store: &SqliteStore) -> String {
    pns_application::DecisionRing::read(store)
        .unwrap()
        .unwrap_or_default()
}
fn snapshot(connection: &mut rusqlite::Connection) -> (bool, String) {
    let transaction = connection.transaction().unwrap();
    let finished: bool = transaction
        .query_row(
            "SELECT finished IS NOT NULL FROM ledger_attempts ORDER BY leg LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let line: String = transaction
        .query_row("SELECT line FROM decisions", [], |row| row.get(0))
        .unwrap();
    (finished, line)
}
mod refusals;
mod verdicts;
mod visibility;
