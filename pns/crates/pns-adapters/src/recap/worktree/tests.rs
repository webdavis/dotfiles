//! What `gh`'s own pull-request listing is read as.

use super::listed;
use pns_domain::recap::git_block::PullRequestLookup;

#[test]
fn a_listing_with_one_pull_request_reads_its_number_and_its_state() {
    // THE STATE COMES BACK IN UPPER CASE and the layout writes it in lower.
    let PullRequestLookup::Found(request) = listed(r#"[{"number":582,"state":"MERGED"}]"#) else {
        panic!("the listing was not read");
    };
    assert_eq!((request.number, request.state.as_str()), (582, "merged"));
}

#[test]
fn an_empty_listing_is_no_pull_request_rather_than_no_answer() {
    assert_eq!(listed("[]\n"), PullRequestLookup::Absent);
}

#[test]
fn anything_that_is_not_the_listing_that_was_asked_for_is_unavailable() {
    // FAIL CLOSED INTO "NOTHING ANSWERED", never into "there is no pull
    // request": a `gh` that printed an error, or that was cut off at the read
    // cap, must not become a recap line saying the branch has no PR.
    for refused in [
        "",
        "no git remote found for the current directory\n",
        r#"[{"number":582,"state":"MERG"#,
        r#"[{"state":"MERGED"}]"#,
        r#"[{"number":582}]"#,
    ] {
        assert_eq!(
            listed(refused),
            PullRequestLookup::Unavailable,
            "{refused:?}"
        );
    }
}
