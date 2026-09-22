//! moshi-hook's own Codex handlers, which reach the phone without pns's
//! presence gate and so are removed wherever they appear.

use serde_json::{Map, Value};
use std::path::Path;

/// Every event's entries with moshi-hook's Codex handlers removed.
///
/// An entry or an event the removal emptied goes with them; one that arrived
/// empty is somebody else's and stays. A shape this cannot walk passes through
/// for the owned-event merge to judge.
pub(super) fn strip_moshi_handlers(hooks: &mut Map<String, Value>) {
    hooks.retain(|_, entries| {
        let Some(entries) = entries.as_array_mut() else {
            return true;
        };
        let arrived = entries.len();
        entries.retain_mut(|entry| {
            let Some(handlers) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = handlers.len();
            handlers.retain(|handler| !is_moshi_handler(handler));
            handlers.len() == before || !handlers.is_empty()
        });
        arrived == 0 || !entries.is_empty()
    });
}

/// Whether this handler's own command is a `moshi-hook` executable called
/// with `codex-hook` as its next word, however the path is spelled or
/// quoted. Only the command word counts: a wrapper that merely names or logs
/// moshi-hook among its own arguments is not moshi-hook itself.
fn is_moshi_handler(handler: &Value) -> bool {
    handler
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| {
            let words = words(command);
            words.first().is_some_and(|first| {
                Path::new(first).file_name().is_some_and(|name| name == "moshi-hook")
            }) && words.get(1).is_some_and(|second| second == "codex-hook")
        })
}

/// The command's shell words, with single and double quotes removed.
fn words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote: Option<char> = None;
    for character in command.chars() {
        match quote {
            Some(open) if character == open => quote = None,
            Some(_) => word.get_or_insert_default().push(character),
            None if character == '\'' || character == '"' => {
                quote = Some(character);
                word.get_or_insert_default();
            }
            None if character.is_whitespace() => words.extend(word.take()),
            None => word.get_or_insert_default().push(character),
        }
    }
    words.extend(word);
    words
}
