use super::*;
use pns_domain::github::{GithubKind, GithubOutcome};

fn extensions(json: &str) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::from_str(json).expect("the test's own JSON parses") {
        serde_json::Value::Object(map) => map,
        other => panic!("expected an object, got {other:?}"),
    }
}

const WELL_FORMED: &str = r#"{"github":{
  "repo":"webdavis/dotfiles","kind":"workflow_run","outcome":"failed",
  "title":"lint","url":"https://github.com/webdavis/dotfiles/actions/runs/1",
  "identity":"webdavis/dotfiles:workflow_run:1","occurred_at":1757000000}}"#;

#[test]
fn a_well_formed_extension_decodes_and_an_envelope_without_one_carries_nothing() {
    assert_eq!(
        github_event(&extensions(WELL_FORMED)),
        Ok(Some(GithubEvent {
            repo: "webdavis/dotfiles".into(),
            kind: GithubKind::WorkflowRun,
            outcome: GithubOutcome::Failed,
            title: "lint".into(),
            url: "https://github.com/webdavis/dotfiles/actions/runs/1".into(),
            identity: "webdavis/dotfiles:workflow_run:1".into(),
            occurred_at: 1_757_000_000,
        }))
    );
    assert_eq!(
        github_event(&extensions(r#"{"other":{"anything":1}}"#)),
        Ok(None),
        "an envelope from any other producer is not a GitHub event"
    );
}

/// EVERY MALFORMED SHAPE IS REFUSED BY NAME rather than read for whatever it
/// holds. A kind or an outcome word this build does not know is the shape that
/// matters most: read as a default it would light a lamp about something
/// nobody mapped.
#[test]
fn every_missing_or_unknown_field_is_refused_by_the_name_that_is_wrong() {
    for (written, named) in [
        (r#"{"github":"a string"}"#, "github"),
        (
            r#"{"github":{"repo":"a/b","kind":"workflow_run"}}"#,
            "outcome",
        ),
        (
            r#"{"github":{"repo":"a/b","kind":"deploy","outcome":"passed","title":"t","url":"u","identity":"i","occurred_at":1}}"#,
            "kind",
        ),
        (
            r#"{"github":{"repo":"a/b","kind":"release","outcome":"cancelled","title":"t","url":"u","identity":"i","occurred_at":1}}"#,
            "outcome",
        ),
        (
            r#"{"github":{"repo":"a/b","kind":"release","outcome":"passed","title":"t","url":"u","identity":"i","occurred_at":-1}}"#,
            "occurred_at",
        ),
        (
            r#"{"github":{"repo":42,"kind":"release","outcome":"passed","title":"t","url":"u","identity":"i","occurred_at":1}}"#,
            "repo",
        ),
    ] {
        let refusal = github_event(&extensions(written)).expect_err("this shape is refused");
        assert!(
            refusal.contains(named),
            "{written} must be refused by `{named}`: {refusal}"
        );
    }
}
