use super::*;

pub(super) fn viewer(answers: Vec<(String, String)>) -> SystemProbes<ExactArgvRunner> {
    SystemProbes::new(
        ExactArgvRunner {
            answers,
            calls: Mutex::new(Vec::new()),
        },
        String::new(),
    )
}

/// Every answer a view of `origin` could want, INCLUDING the two
/// caller-relative ones: a test that tells the two readings apart has to
/// let the wrong reading succeed rather than fail for want of an answer.
pub(super) fn answers(workspace_list: &str, origin: &str, layout: &str) -> Vec<(String, String)> {
    vec![
        (
            "herdr workspace list".to_string(),
            workspace_list.to_string(),
        ),
        (
            format!("herdr pane layout --pane {origin}"),
            layout.to_string(),
        ),
        (
            "herdr pane current".to_string(),
            PANE_CURRENT_CALLER_RELATIVE.to_string(),
        ),
        (
            format!("herdr pane get {origin}"),
            PANE_CURRENT_CALLER_RELATIVE.to_string(),
        ),
    ]
}
