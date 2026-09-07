/// Resolve the pane to act on. A plugin action receives HERDR_PANE_ID; the old
/// shell keybinding (and manual use) set HERDR_ACTIVE_PANE_ID. Prefer the former,
/// fall back to the latter, ignore empty strings.
pub(crate) fn resolve_pane(pane_id: Option<String>, active: Option<String>) -> Option<String> {
    [pane_id, active]
        .into_iter()
        .flatten()
        .find(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_pane_prefers_pane_id() {
        assert_eq!(
            resolve_pane(Some("p1".into()), Some("p2".into())),
            Some("p1".into())
        );
    }

    #[test]
    fn resolve_pane_falls_back_to_active_when_pane_id_missing_or_empty() {
        assert_eq!(resolve_pane(None, Some("p2".into())), Some("p2".into()));
        assert_eq!(
            resolve_pane(Some(String::new()), Some("p2".into())),
            Some("p2".into())
        );
    }

    #[test]
    fn resolve_pane_none_when_all_absent_or_empty() {
        assert_eq!(resolve_pane(None, None), None);
        assert_eq!(resolve_pane(Some(String::new()), Some(String::new())), None);
    }
}
