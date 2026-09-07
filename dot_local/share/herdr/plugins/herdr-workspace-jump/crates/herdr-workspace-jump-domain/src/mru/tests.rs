use super::*;

fn mru(current: &str, previous: &str) -> Mru {
    Mru {
        current: current.to_string(),
        previous: previous.to_string(),
    }
}

fn workspace(workspace_id: &str) -> Workspace {
    Workspace {
        workspace_id: workspace_id.to_string(),
        label: String::new(),
    }
}

#[test]
fn next_mru_on_a_cold_start_records_only_the_current() {
    assert_eq!(next_mru(&mru("", ""), "A"), Some(mru("A", "")));
}

#[test]
fn next_mru_keeps_a_previous_it_has_no_current_to_replace() {
    // Reachable through a truncated or hand-edited state file, where
    // `read_at` yields an empty current beside a recorded previous. The
    // recorded one is still the better toggle target than nothing.
    assert_eq!(next_mru(&mru("", "wB"), "A"), Some(mru("A", "wB")));
}

#[test]
fn next_mru_shifts_the_workspace_being_left_into_previous() {
    assert_eq!(next_mru(&mru("A", ""), "B"), Some(mru("B", "A")));
    assert_eq!(next_mru(&mru("B", "A"), "C"), Some(mru("C", "B")));
}

#[test]
fn next_mru_ignores_a_refocus_of_the_current_or_an_empty_id() {
    assert_eq!(next_mru(&mru("B", "A"), "B"), None);
    assert_eq!(next_mru(&mru("B", "A"), ""), None);
}

#[test]
fn decide_bounce_focuses_a_previous_that_still_exists() {
    let workspaces = [workspace("wA"), workspace("wB")];
    assert_eq!(
        decide_bounce(&mru("wA", "wB"), &workspaces),
        Bounce::Focus("wB".to_string())
    );
}

#[test]
fn decide_bounce_drops_a_previous_that_is_gone() {
    assert_eq!(
        decide_bounce(&mru("wA", "wGone"), &[workspace("wA")]),
        Bounce::DropStale
    );
}

#[test]
fn decide_bounce_does_nothing_before_anything_is_recorded() {
    assert_eq!(
        decide_bounce(&mru("wA", ""), &[workspace("wA")]),
        Bounce::Nothing
    );
    assert_eq!(decide_bounce(&mru("", ""), &[]), Bounce::Nothing);
}
