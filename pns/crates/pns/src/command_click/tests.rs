use super::*;

/// This binary's own path, never a bare name: a click has no PATH to resolve
/// with, and a machine mid-upgrade can have two pns on disk.
#[test]
fn the_view_runs_this_binary_rather_than_whatever_a_path_would_find() {
    let path = pns_path();
    assert!(path.starts_with('/'), "{path}");
    assert_ne!(path, "pns");
}

/// The whole of what `pns click` is for, end to end through the domain: the
/// argv opens THIS binary's detail view for the id the banner carried.
#[test]
fn the_inferred_view_opens_this_binarys_detail_view_for_that_id() {
    let argv = ClickView::inferred(false)
        .argv(47, "/bin/herdr", &pns_path())
        .unwrap();
    assert_eq!(argv[0], "/usr/bin/open");
    assert_eq!(argv.last().unwrap(), &format!("{} failures 47", pns_path()));
}

/// A usage line names the one argument, so an operator who runs it by hand while
/// diagnosing a click that did nothing is told what it wanted.
#[test]
fn the_usage_line_names_the_single_argument() {
    assert!(CLICK_USAGE.contains("pns click"), "{CLICK_USAGE}");
    assert!(CLICK_USAGE.contains("<failure-id>"), "{CLICK_USAGE}");
}
