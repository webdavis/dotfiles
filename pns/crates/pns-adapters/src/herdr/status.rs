/// One `agent_status` per workspace, in the order herdr listed them, with a
/// workspace that carries no such field answering the EMPTY string.
///
/// A MISSING FIELD IS NOT A WORKING LOOP, which is the fail-toward-dark
/// direction this whole design takes, and it is not hypothetical: the suite's
/// own shipped herdr stub answers a `workspace list` with no `agent_status` in
/// it, and a herdr that stops carrying the field must leave a lamp dark rather
/// than breathing forever.
///
/// A SECOND READER OF ONE ANSWER, not a change to `parse_focused_tab`: that
/// function reads `focused` and `active_tab_id` for the visibility model and
/// has no business knowing what a lamp does.
pub fn workspace_agent_statuses(workspace_list_json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()
        .as_ref()
        .and_then(|body| body.pointer("/result/workspaces"))
        .and_then(serde_json::Value::as_array)
        .map(|workspaces| {
            workspaces
                .iter()
                .map(|workspace| {
                    workspace
                        .get("agent_status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
