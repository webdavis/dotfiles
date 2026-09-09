/// The tab the SESSION is showing, from `workspace list`: the active tab of
/// the one workspace flagged focused.
///
/// This is the only session-global answer herdr gives. No workspace flagged
/// focused is None, which becomes an unreadable view rather than a guess.
pub fn parse_focused_tab(workspace_list_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()?
        .pointer("/result/workspaces")?
        .as_array()?
        .iter()
        .find(|workspace| workspace.get("focused").and_then(|f| f.as_bool()) == Some(true))?
        .get("active_tab_id")?
        .as_str()
        .map(str::to_string)
}

/// One tab's arrangement, from `pane layout`.
#[derive(Debug, PartialEq)]
pub struct TabLayout {
    /// The tab this layout describes, which is the tab holding whichever pane
    /// the call addressed.
    pub tab_id: String,
    /// The focused pane WITHIN this tab. Tab-level truth, not the caller's
    /// pane: every pane in a tab is answered the same focused pane id.
    pub focused_pane: String,
    /// ZOOM IS TAB-LEVEL: one pane fills the window and every sibling is off
    /// screen.
    pub zoomed: bool,
}

/// A tab's arrangement, addressed by any pane inside it. The pane list is not
/// read: visibility turns on the focused pane and the zoom flag alone.
///
/// A missing field is a shape we do not know, and the whole reading is
/// refused rather than half-trusted: assuming a tab is unzoomed suppresses a
/// notification the operator cannot see.
pub fn parse_layout(layout_json: &str) -> Option<TabLayout> {
    let layout = serde_json::from_str::<serde_json::Value>(layout_json)
        .ok()?
        .pointer("/result/layout")?
        .clone();
    Some(TabLayout {
        tab_id: layout.get("tab_id")?.as_str()?.to_string(),
        focused_pane: layout.get("focused_pane_id")?.as_str()?.to_string(),
        zoomed: layout.get("zoomed")?.as_bool()?,
    })
}
