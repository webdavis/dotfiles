use posture_application::WatchdogState;
use posture_domain::{Agent, AgentState, AuditMemory, QueueMemory};
use serde_json::{Value, json};

pub(super) fn decode(bytes: &[u8]) -> Option<WatchdogState> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    value.as_object()?;
    let mut state = WatchdogState::default();
    for (index, agent) in Agent::ALL.into_iter().enumerate() {
        if let Some(entry) = value["agents"][agent.label()].as_object() {
            state.agents[index] = Some(AgentState {
                runs: entry.get("runs").and_then(number),
                streak: entry.get("streak").and_then(number).unwrap_or(0),
            });
        }
    }
    state.legacy_pending = queue(&value["pending"]);
    state.pns_pending = queue(&value["pns_pending"]);
    let audit = &value["pipeline_audit"];
    state.pipeline_audit = AuditMemory::from_readings(
        audit["fingerprint"].as_str().unwrap_or(""),
        &audit["streak"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| audit["streak"].to_string()),
        audit["paged_fingerprint"].as_str().unwrap_or(""),
    );
    Some(state)
}
fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let text = value.as_str()?;
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    })
}
fn queue(value: &Value) -> QueueMemory {
    QueueMemory {
        count: number(&value["count"]),
        growth_streak: number(&value["growth_streak"]).unwrap_or(0),
        // A state file written before dead letters were remembered carries no
        // key here, which reads as "never observed" and pages the standing
        // count once on the first tick after the upgrade.
        deadletters: number(&value["deadletters"]),
    }
}
pub(super) fn encode(state: &WatchdogState) -> Vec<u8> {
    let mut agents = serde_json::Map::new();
    for (agent, memory) in Agent::ALL.into_iter().zip(state.agents) {
        if let Some(memory) = memory {
            agents.insert(agent.label().into(), json!({"runs": memory.runs.map_or(json!(-1), |n| json!(n)), "streak": memory.streak}));
        }
    }
    let queue = |memory: QueueMemory| json!({"count": memory.count.map_or(json!(-1), |n| json!(n)), "growth_streak": memory.growth_streak, "deadletters": memory.deadletters.map_or(json!(-1), |n| json!(n))});
    let audit = &state.pipeline_audit;
    let mut bytes = json!({
        "agents": agents,
        "pending": queue(state.legacy_pending),
        "pns_pending": queue(state.pns_pending),
        "pipeline_audit": {"fingerprint": audit.fingerprint.as_ref().map_or("", |v| v.as_str()), "streak": audit.streak.min(99), "paged_fingerprint": audit.paged.as_ref().map_or("", |v| v.as_str())}
    }).to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}
