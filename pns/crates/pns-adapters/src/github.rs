//! The `github` extension on a producer request, read into the domain event.
//!
//! THE ENVELOPE CARRIES IT VERBATIM. `extensions` is documented as
//! "producer-specific data, carried verbatim and never read here", so the
//! decode belongs at an edge rather than in the protocol crate, and the type
//! it answers with is the domain's.

use pns_domain::github::{GITHUB_KIND_WORDS, GITHUB_OUTCOME_WORDS, GithubEvent};
use serde_json::{Map, Value};

/// The extension key a GitHub producer writes under.
pub const GITHUB_EXTENSION: &str = "github";

/// The GitHub event this request carries, nothing at all when it carries no
/// `github` extension, or a refusal naming the field that is wrong.
///
/// A MALFORMED EXTENSION IS A REFUSAL, NEVER A DEFAULT. An unknown kind or
/// outcome word read as a nearest match would light a lamp about something
/// nobody mapped, which is the same silence the closed enums exist to prevent.
pub fn github_event(extensions: &Map<String, Value>) -> Result<Option<GithubEvent>, String> {
    let Some(stated) = extensions.get(GITHUB_EXTENSION) else {
        return Ok(None);
    };
    let Some(table) = stated.as_object() else {
        return Err(format!(
            "`extensions.github` is a {}, not an object",
            kind_of(stated)
        ));
    };
    Ok(Some(GithubEvent {
        repo: text(table, "repo")?,
        kind: word(table, "kind", &GITHUB_KIND_WORDS)?,
        outcome: word(table, "outcome", &GITHUB_OUTCOME_WORDS)?,
        title: text(table, "title")?,
        url: text(table, "url")?,
        identity: text(table, "identity")?,
        occurred_at: epoch(table, "occurred_at")?,
    }))
}

/// This event as the `extensions` map a producer request carries it in.
///
/// THE ENCODE LIVES BESIDE THE DECODE, so one module owns both halves of the
/// extension and a round-trip test pins them to each other: a field renamed
/// on one side and not the other is red here rather than a colour nobody
/// could read at the far end.
///
/// THE WORDS COME OFF THE DOMAIN'S OWN TABLES, which is what makes the two
/// halves share one vocabulary rather than two spellings of it.
pub fn github_extensions(event: &GithubEvent) -> Map<String, Value> {
    let mut extensions = Map::new();
    extensions.insert(
        GITHUB_EXTENSION.to_string(),
        serde_json::json!({
            "repo": event.repo,
            "kind": word_for(event.kind, &GITHUB_KIND_WORDS),
            "outcome": word_for(event.outcome, &GITHUB_OUTCOME_WORDS),
            "title": event.title,
            "url": event.url,
            "identity": event.identity,
            "occurred_at": event.occurred_at,
        }),
    );
    extensions
}

/// One closed-vocabulary value's own spelling.
fn word_for<T: PartialEq>(value: T, vocabulary: &[(&'static str, T)]) -> &'static str {
    vocabulary
        .iter()
        .find(|(_, mapped)| *mapped == value)
        .map_or("", |(word, _)| *word)
}

/// One required string.
fn text(table: &Map<String, Value>, field: &str) -> Result<String, String> {
    match table.get(field) {
        Some(Value::String(text)) => Ok(text.clone()),
        other => Err(missing(field, "a string", other)),
    }
}

/// One required epoch second, which is a whole number and never a negative
/// one: a GitHub event before 1970 is a clock nobody should be trusted about.
fn epoch(table: &Map<String, Value>, field: &str) -> Result<u64, String> {
    match table.get(field).map(|stated| (stated, stated.as_u64())) {
        Some((_, Some(seconds))) => Ok(seconds),
        other => Err(missing(
            field,
            "a whole number of epoch seconds",
            other.map(|(stated, _)| stated),
        )),
    }
}

/// One required word out of a closed vocabulary, refused with the vocabulary
/// named so the operator reads what this build accepts.
fn word<T: Copy>(
    table: &Map<String, Value>,
    field: &str,
    vocabulary: &[(&str, T)],
) -> Result<T, String> {
    let stated = text(table, field)?;
    vocabulary
        .iter()
        .find(|(word, _)| *word == stated)
        .map(|(_, value)| *value)
        .ok_or_else(|| {
            let known: Vec<&str> = vocabulary.iter().map(|(word, _)| *word).collect();
            format!(
                "`extensions.github.{field}` is `{stated}`, which is none of {}",
                known.join(", ")
            )
        })
}

/// The refusal one absent or wrongly typed field answers with.
fn missing(field: &str, wanted: &str, stated: Option<&Value>) -> String {
    match stated {
        Some(stated) => format!(
            "`extensions.github.{field}` is a {}, not {wanted}",
            kind_of(stated)
        ),
        None => format!("`extensions.github.{field}` is missing; it is {wanted}"),
    }
}

/// What a value is, in the words a refusal uses.
fn kind_of(stated: &Value) -> &'static str {
    match stated {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub mod client;
pub mod notifications;
pub mod poll_state;

#[cfg(test)]
mod tests;
