//! pns's four hooks, merged into `~/.codex/hooks.json`.
//!
//! HERDR OWNS ITS OWN ENTRY and regenerates it on update, so the merge adds
//! and rewrites only the handlers pns itself generated and copies every other
//! handler through untouched. moshi-hook's own Codex handlers are the one
//! exception: they are removed, so moshi hears from Codex only through pns.
//!
//! A FILE THIS CANNOT READ IS REFUSED RATHER THAN REPLACED: an operator's
//! broken-but-recoverable hooks file is worth more than this tool's guess at
//! what it meant.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

mod moshi;
#[cfg(test)]
mod tests;

/// One Codex event pns owns, and the hook the binary runs for it.
struct Owned {
    event: &'static str,
    action: &'static str,
    /// What the current command carries after the hook word.
    flags: &'static str,
}

/// The four events, in the order a fresh file gains them.
///
/// PostToolUse carries no matcher, unlike the narrowed pair on Claude Code:
/// Codex has no batch event to move the broad row to, so every tool call pays
/// a cheap synchronous resolved spawn.
const OWNED: &[Owned] = &[
    Owned {
        event: "Stop",
        action: "stop",
        flags: "",
    },
    Owned {
        event: "PermissionRequest",
        action: "blocked",
        flags: " --remind=5m",
    },
    // The answered signal, on both events that end a wait: PostToolUse fires
    // once a tool has produced output, including a non-zero exit, and Interrupt
    // fires when the operator ends the turn instead of answering.
    Owned {
        event: "PostToolUse",
        action: "resolved",
        flags: "",
    },
    Owned {
        event: "Interrupt",
        action: "resolved",
        flags: "",
    },
];

/// Whether the merge had anything to change.
#[derive(Debug, PartialEq, Eq)]
pub enum CodexHooksInstall {
    /// The document gained or changed a handler, which Codex ignores until the
    /// operator trusts it.
    Changed,
    /// The file already said what this run would have written.
    Unchanged,
}

/// Where Codex keeps the hooks this merges into.
pub fn codex_hooks_path(home: &str) -> PathBuf {
    Path::new(home).join(".codex").join("hooks.json")
}

/// Read, merge and write, leaving the file untouched on any refusal.
pub fn install_codex_hooks(home: &str, binary: &str) -> Result<CodexHooksInstall, String> {
    let path = codex_hooks_path(home);
    let base = read_document(&path)?;
    let merged = merge_codex_hooks(&base, home, binary)?;
    write_document(&path, &merged)?;
    Ok(if merged == base {
        CodexHooksInstall::Unchanged
    } else {
        CodexHooksInstall::Changed
    })
}

/// pns's four events, merged into the document the caller read, with
/// moshi-hook's Codex handlers removed.
///
/// PURE, so the whole merge is exercised without a filesystem.
pub fn merge_codex_hooks(base: &Value, home: &str, binary: &str) -> Result<Value, String> {
    let mut merged = base.clone();
    let hooks = merged
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "pns: the Codex hooks document has no object \"hooks\" field".to_string())?;
    moshi::strip_moshi_handlers(hooks);
    for owned in OWNED {
        let command = current_command(binary, owned);
        let entries = merge_event(
            hooks.get(owned.event),
            &command,
            &spellings(home, owned, &command),
            owned.event,
        )?;
        hooks.insert(owned.event.to_string(), Value::Array(entries));
    }
    Ok(merged)
}

/// What this run installs for one event.
fn current_command(binary: &str, owned: &Owned) -> String {
    format!(
        "PNS_PRODUCER=codex {binary} hook {}{}",
        owned.action, owned.flags
    )
}

/// Every complete command this installer has ever generated for one event.
///
/// KEEPING THE RETIRED SPELLINGS is what rewrites a deployed row in place
/// rather than leaving it beside the new one.
fn spellings(home: &str, owned: &Owned, current: &str) -> Vec<String> {
    let legacy_action = if owned.action == "stop" {
        "done"
    } else {
        owned.action
    };
    let mut spellings = vec![current.to_string()];
    if owned.action == "blocked" {
        spellings.push(current.replace(" --remind=5m", " --remind"));
    }
    for variable in ["PNS_PRODUCER", "PNS_AGENT", "RELAY_AGENT"] {
        for engine in [".cargo/bin/pns", ".local/libexec/pns/pns"] {
            spellings.push(format!(
                "{variable}=codex {home}/{engine} hook {}",
                owned.action
            ));
        }
    }
    for script in [
        ".local/bin/relay-agent.sh",
        ".local/libexec/pns/codex-hooks/relay-agent.sh",
        ".local/libexec/pns/hooks/relay-agent.sh",
    ] {
        spellings.push(format!("RELAY_AGENT=codex {home}/{script} {legacy_action}"));
    }
    spellings.push(format!(
        "PNS_AGENT=codex {home}/.local/libexec/pns/hooks/relay-agent.sh {legacy_action}"
    ));
    spellings
}

/// One event's entries, with pns's handler rewritten in place and every
/// foreign handler carried through.
fn merge_event(
    existing: Option<&Value>,
    command: &str,
    owned: &[String],
    event: &str,
) -> Result<Vec<Value>, String> {
    let existing = match existing {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(entries)) => entries.as_slice(),
        Some(_) => return Err(malformed(event)),
    };
    let mut kept_entries: Vec<Value> = Vec::new();
    let mut owner: Option<Value> = None;
    for entry in existing {
        let object = entry.as_object().ok_or_else(|| malformed(event))?;
        let handlers = object
            .get("hooks")
            .and_then(Value::as_array)
            .ok_or_else(|| malformed(event))?;
        let mut kept: Vec<Value> = Vec::new();
        for handler in handlers {
            if !is_owned(handler, owned) {
                kept.push(handler.clone());
                continue;
            }
            let mut updated = handler.clone();
            updated["command"] = Value::String(command.to_string());
            let identity = identity(object, &updated);
            match &owner {
                None => {
                    owner = Some(identity);
                    kept.push(updated);
                }
                // A second row saying exactly what the first says is a
                // duplicate, and one owner is what gets kept.
                Some(first) if *first == identity => {}
                Some(_) => {
                    return Err(format!(
                        "pns: the {event} hook's metadata differs between two pns handlers; \
leaving the Codex hooks untouched"
                    ));
                }
            }
        }
        // An entry the rewrite emptied is dropped; one that arrived empty is
        // somebody else's and stays.
        if !kept.is_empty() || handlers.is_empty() {
            let mut rewritten = object.clone();
            rewritten.insert("hooks".to_string(), Value::Array(kept));
            kept_entries.push(Value::Object(rewritten));
        }
    }
    if owner.is_none() {
        kept_entries.push(json!({"hooks": [{"type": "command", "command": command}]}));
    }
    Ok(kept_entries)
}

/// Whether this handler is one this installer generated.
fn is_owned(handler: &Value, owned: &[String]) -> bool {
    handler.get("type").and_then(Value::as_str) == Some("command")
        && handler
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| owned.iter().any(|spelling| spelling == command))
}

/// What two pns handlers have to agree on to be one owner: the entry's own
/// metadata and the handler's, the command apart.
fn identity(entry: &Map<String, Value>, handler: &Value) -> Value {
    let mut group = entry.clone();
    group.remove("hooks");
    json!({"group": Value::Object(group), "handler": handler})
}

fn malformed(event: &str) -> String {
    format!(
        "pns: the {event} hook is not a list of entries carrying a list of handlers; leaving the Codex hooks untouched"
    )
}

/// The document on disk, or the empty one an absent, empty or blank file
/// stands for.
fn read_document(path: &Path) -> Result<Value, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("pns: cannot read {}: {error}", path.display())),
    };
    if raw.trim().is_empty() {
        return Ok(json!({"hooks": {}}));
    }
    let parsed: Value = serde_json::from_str(&raw).map_err(|_| unreadable(path))?;
    if !parsed.is_object() || !parsed.get("hooks").is_some_and(Value::is_object) {
        return Err(unreadable(path));
    }
    Ok(parsed)
}

fn unreadable(path: &Path) -> String {
    format!(
        "pns: {} is not a single JSON object with an object \"hooks\" field; leaving it untouched",
        path.display()
    )
}

/// Publish by rename, so a reader sees the old document or the new one.
fn write_document(path: &Path, document: &Value) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| format!("pns: {} names no directory to write into", path.display()))?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("pns: cannot create {}: {error}", directory.display()))?;
    let text = serde_json::to_string_pretty(document)
        .map_err(|error| format!("pns: cannot render the Codex hooks: {error}"))?;
    let pending = directory.join(format!(".hooks.json.{}", std::process::id()));
    std::fs::write(&pending, format!("{text}\n"))
        .map_err(|error| format!("pns: cannot write {}: {error}", pending.display()))?;
    std::fs::rename(&pending, path).map_err(|error| {
        let _ = std::fs::remove_file(&pending);
        format!("pns: cannot publish {}: {error}", path.display())
    })
}
