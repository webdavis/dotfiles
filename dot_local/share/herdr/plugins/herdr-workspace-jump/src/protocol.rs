//! The herdr wire format, shared by both directories.
//!
//! herdr speaks newline-delimited JSON over its local socket: one request per
//! line, one response line carrying the same `id`. The CLI prints that same
//! envelope on stdout, so the response half is shared by the socket directory
//! and the CLI fallback rather than written twice.

use std::fmt;

use serde_json::{Value, json};

/// A workspace, reduced to the two fields a jump needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub workspace_id: String,
    pub label: String,
}

/// Why a jump could not be completed.
///
/// These are distinguished rather than collapsed into one string because the
/// composition root routes on them: `Unreachable` and `Transport` mean the
/// socket never produced an answer, which is what the CLI fallback is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumpError {
    /// herdr could not be reached at all: no socket, or no CLI binary.
    Unreachable(String),
    /// A read or write failed after connecting, or the deadline expired.
    Transport(String),
    /// herdr answered with something this plugin cannot read.
    Malformed(String),
    /// herdr answered with an error envelope.
    Server { code: String, message: String },
    /// Both the socket and the CLI fallback failed. Rendered rather than
    /// nested, so the combined message keeps one prefix instead of stacking
    /// the wrapper's on top of the wrapped one's.
    BothPathsFailed { cli: String, socket: String },
}

impl fmt::Display for JumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(detail) => write!(f, "herdr unreachable: {detail}"),
            Self::Transport(detail) => write!(f, "herdr transport failed: {detail}"),
            Self::Malformed(detail) => write!(f, "unreadable herdr response: {detail}"),
            Self::Server { code, message } => {
                write!(f, "herdr refused the request: {code}: {message}")
            }
            Self::BothPathsFailed { cli, socket } => {
                write!(f, "{cli} (the socket was tried first: {socket})")
            }
        }
    }
}

/// Encode one `workspace.list` request line.
pub fn list_request(request_id: &str) -> String {
    request_line(request_id, "workspace.list", json!({}))
}

/// Encode one `workspace.focus` request line.
pub fn focus_request(request_id: &str, workspace_id: &str) -> String {
    request_line(
        request_id,
        "workspace.focus",
        json!({ "workspace_id": workspace_id }),
    )
}

/// Encode one `workspace.create` request line. `focus` is always true: this
/// plugin creates a workspace only in order to jump into it.
pub fn create_request(request_id: &str, label: &str, cwd: &str) -> String {
    request_line(
        request_id,
        "workspace.create",
        json!({ "cwd": cwd, "label": label, "focus": true }),
    )
}

fn request_line(request_id: &str, method: &str, params: Value) -> String {
    let request = json!({ "id": request_id, "method": method, "params": params });
    format!("{request}\n")
}

/// Unwrap a response envelope into its `result` object.
///
/// An empty body is accepted as success with an empty result: `herdr workspace
/// focus` prints nothing on some paths, and the caller only needs the result
/// for `workspace.list`.
pub fn result_of(body: &str) -> Result<Value, JumpError> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    let envelope: Value = serde_json::from_str(body.trim())
        .map_err(|error| JumpError::Malformed(format!("not JSON: {error}")))?;
    if let Some(failure) = envelope.get("error") {
        return Err(JumpError::Server {
            code: string_at(failure, "code").unwrap_or_else(|| "unknown".to_string()),
            message: string_at(failure, "message").unwrap_or_else(|| body.trim().to_string()),
        });
    }
    match envelope.get("result") {
        Some(result) => Ok(result.clone()),
        None => Err(JumpError::Malformed(
            "envelope has neither result nor error".to_string(),
        )),
    }
}

/// Read the workspace records out of a `workspace.list` result.
///
/// A record with no label keeps an empty one rather than being dropped: an
/// empty label can never equal a chord's label, so it falls out of the match on
/// its own, and dropping it would hide a malformed record instead.
pub fn workspaces_of(result: &Value) -> Result<Vec<Workspace>, JumpError> {
    let records = result
        .get("workspaces")
        .and_then(Value::as_array)
        .ok_or_else(|| JumpError::Malformed("result has no workspaces array".to_string()))?;
    records
        .iter()
        .map(|record| {
            let workspace_id = string_at(record, "workspace_id").ok_or_else(|| {
                JumpError::Malformed("a workspace record has no workspace_id".to_string())
            })?;
            Ok(Workspace {
                workspace_id,
                label: string_at(record, "label").unwrap_or_default(),
            })
        })
        .collect()
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> Value {
        match serde_json::from_str(line.trim()) {
            Ok(value) => value,
            Err(error) => panic!("encoder produced invalid JSON: {error}"),
        }
    }

    #[test]
    fn every_request_is_one_json_line_terminated_by_a_newline() {
        for line in [
            list_request("1"),
            focus_request("2", "wA"),
            create_request("3", "dotfiles", "/tmp/x"),
        ] {
            assert!(line.ends_with('\n'), "missing newline: {line}");
            assert_eq!(line.matches('\n').count(), 1, "embedded newline: {line}");
            assert!(parsed(&line).is_object());
        }
    }

    #[test]
    fn list_request_names_the_method_and_carries_the_id() {
        let request = parsed(&list_request("req_1"));
        assert_eq!(request["method"], "workspace.list");
        assert_eq!(request["id"], "req_1");
    }

    #[test]
    fn focus_request_carries_the_workspace_id() {
        let request = parsed(&focus_request("req_2", "w11"));
        assert_eq!(request["method"], "workspace.focus");
        assert_eq!(request["params"]["workspace_id"], "w11");
    }

    #[test]
    fn create_request_carries_cwd_label_and_always_focuses() {
        let request = parsed(&create_request("req_3", "netpulse", "/home/me/netpulse"));
        assert_eq!(request["method"], "workspace.create");
        assert_eq!(request["params"]["cwd"], "/home/me/netpulse");
        assert_eq!(request["params"]["label"], "netpulse");
        assert_eq!(request["params"]["focus"], true);
    }

    #[test]
    fn create_request_escapes_a_label_that_would_break_the_line() {
        let request = create_request("4", "odd \"quoted\"\nlabel", "/tmp");
        assert_eq!(request.matches('\n').count(), 1);
        assert_eq!(parsed(&request)["params"]["label"], "odd \"quoted\"\nlabel");
    }

    #[test]
    fn result_of_unwraps_a_success_envelope() {
        let result = result_of(r#"{"id":"1","result":{"type":"pong"}}"#);
        assert_eq!(result, Ok(json!({"type": "pong"})));
    }

    #[test]
    fn result_of_treats_an_empty_body_as_an_empty_success() {
        assert_eq!(result_of(""), Ok(json!({})));
        assert_eq!(result_of("  \n "), Ok(json!({})));
    }

    #[test]
    fn result_of_reports_an_error_envelope_as_a_server_failure() {
        let failure =
            result_of(r#"{"id":"1","error":{"code":"not_found","message":"no such workspace"}}"#);
        assert_eq!(
            failure,
            Err(JumpError::Server {
                code: "not_found".to_string(),
                message: "no such workspace".to_string(),
            })
        );
    }

    #[test]
    fn result_of_rejects_non_json_and_an_envelope_with_neither_arm() {
        assert!(matches!(
            result_of("not json"),
            Err(JumpError::Malformed(_))
        ));
        assert!(matches!(
            result_of(r#"{"id":"1"}"#),
            Err(JumpError::Malformed(_))
        ));
    }

    #[test]
    fn workspaces_of_reads_id_and_label() {
        let result = json!({"workspaces": [
            {"workspace_id": "wW", "label": "dotfiles modernization"},
            {"workspace_id": "w11", "label": "dotfiles"}
        ]});
        assert_eq!(
            workspaces_of(&result),
            Ok(vec![
                Workspace {
                    workspace_id: "wW".to_string(),
                    label: "dotfiles modernization".to_string(),
                },
                Workspace {
                    workspace_id: "w11".to_string(),
                    label: "dotfiles".to_string(),
                },
            ])
        );
    }

    #[test]
    fn workspaces_of_keeps_a_labelless_record_with_an_empty_label() {
        let result =
            json!({"workspaces": [{"workspace_id": "wA"}, {"workspace_id": "wB", "label": null}]});
        let workspaces = match workspaces_of(&result) {
            Ok(workspaces) => workspaces,
            Err(error) => panic!("expected the records to parse: {error}"),
        };
        assert_eq!(workspaces.len(), 2);
        assert!(
            workspaces
                .iter()
                .all(|workspace| workspace.label.is_empty())
        );
    }

    #[test]
    fn workspaces_of_rejects_a_missing_array_or_a_record_without_an_id() {
        assert!(matches!(
            workspaces_of(&json!({})),
            Err(JumpError::Malformed(_))
        ));
        assert!(matches!(
            workspaces_of(&json!({"workspaces": [{"label": "x"}]})),
            Err(JumpError::Malformed(_))
        ));
    }

    #[test]
    fn every_error_renders_on_one_line() {
        for error in [
            JumpError::Unreachable("no such file".to_string()),
            JumpError::Transport("timed out".to_string()),
            JumpError::Malformed("not JSON".to_string()),
            JumpError::Server {
                code: "not_found".to_string(),
                message: "gone".to_string(),
            },
            JumpError::BothPathsFailed {
                cli: "herdr unreachable: no binary".to_string(),
                socket: "herdr unreachable: no socket".to_string(),
            },
        ] {
            let rendered = error.to_string();
            assert!(!rendered.contains('\n'), "multi-line: {rendered}");
            assert!(!rendered.is_empty());
        }
    }
}
