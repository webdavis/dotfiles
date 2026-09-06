//! The nag's on-disk shape: one record as JSON, and the paths its files live
//! at under a state directory the composition root owns.
//!
//! POLICY ONLY here too, in `decision_log.rs`'s style: no config, no clock, no
//! environment and no printing. What a record MEANS moved to `pns-domain`; the
//! codec stays because this crate is where `serde_json` lives, and the paths
//! stay because they are resolved against a directory only the root knows.

pub use pns_domain::nag::{
    Dropped, FIRE_STALE_SECS, Fate, MAX_SESSION_ID_CHARS, RECORD_SUFFIX, Record, fate, is_stale,
    job_id, marker_name, nudge, session_of, usable, waited,
};

/// One record as one JSON object, `missed_notifications::entry`'s shape.
///
/// JSON AND NOT `key=value`, for the journal's own reason: the detail is a
/// permission prompt's text and can carry a newline, a tab or a quote, and a
/// line-oriented form would let one of those forge a second record. BUILT WITH
/// `json!` AND NEVER WITH `format!`, which is this repo's "build JSON with
/// `jq -n --arg`" rule in Rust.
pub fn render(record: &Record) -> String {
    serde_json::json!({
        "agent": record.agent,
        "project": record.project,
        "branch": record.branch,
        "detail": record.detail,
        "pane": record.pane,
        "armed": record.armed,
    })
    .to_string()
}

/// That object read back, or None for text that is not one.
///
/// PARSED BY KEY, never by position, which is the journal's own rule: the
/// writer's key order belongs to `serde_json` and no reader should depend on
/// it.
///
/// A MISSING KEY READS AS EMPTY rather than refusing the record, again
/// following the journal: a short record degrades to a thinner card, and every
/// value here already has an empty reading. A missing `armed` is second zero,
/// which the staleness cap then refuses as far too old, so the degraded case
/// resolves to silence rather than to a nudge about an unknown moment.
///
/// TEXT THAT IS NOT A JSON OBJECT IS REFUSED, because there is nothing to
/// degrade to: a file somebody else wrote at this path is not a thinner record
/// of ours, and the fire drops the claim rather than guessing at it.
pub fn parse(text: &str) -> Option<Record> {
    let fields: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text).ok()?;
    Some(Record {
        agent: string(&fields, "agent"),
        project: string(&fields, "project"),
        branch: string(&fields, "branch"),
        detail: string(&fields, "detail"),
        pane: string(&fields, "pane"),
        armed: fields
            .get("armed")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
    })
}

/// One string field, or empty when the key is absent or holds something else.
fn string(fields: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

// --- the names --------------------------------------------------------------

/// Where the records live: a SUBDIRECTORY of the state directory, deliberately.
///
/// The state directory is otherwise flat, but the fire ENUMERATES records, and
/// a flat directory would mean pattern-matching every other state file on every
/// wake. The daemon's own `daemon/` and `daemon-markers/` set the precedent.
pub fn nag_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("nag")
}

/// The record's path for one session, or None for an id that may not be a
/// filename.
pub fn record_path(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    usable(session_id).map(|id| nag_dir(state_dir).join(format!("{id}{RECORD_SUFFIX}")))
}

/// The name this process claims one record under, BUILT FROM THE WHOLE FILE
/// NAME.
///
/// NEVER `Path::with_extension`, which replaces everything after the LAST dot.
/// A harness session id may contain dots, so a claim derived from anything
/// short of the full name can collapse two sessions onto one claim: one loses
/// its nudge and the other can be delivered twice. Appending to the whole name
/// cannot, whatever the id contains.
///
/// THE RENAME IS THE OWNERSHIP TEST and this is the name it renames to. A plain
/// unlink does not arbitrate on this filesystem, which is why the fire takes a
/// record by rename before reading it for anything. The measurement behind that
/// is in `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`.
pub fn claim_path(record: &std::path::Path, pid: u32) -> std::path::PathBuf {
    let name = record.file_name().unwrap_or_default().to_string_lossy();
    record.with_file_name(format!("{name}{CLAIM_INFIX}{pid}"))
}

/// What a held claim's name carries before the pid.
const CLAIM_INFIX: &str = ".claim.";

/// The whole FIRE's lock, one well-known name beside the records.
///
/// NOT A RECORD NAME, so it can never be enumerated as one: a record ends in
/// `RECORD_SUFFIX` and this does not, and neither does the claim taken from it.
pub const FIRE_LOCK: &str = "fire.lock";

#[cfg(test)]
mod tests;
