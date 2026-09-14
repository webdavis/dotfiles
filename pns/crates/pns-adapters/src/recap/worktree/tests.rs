//! What gh-axi's own listing format is read as.

use super::listed;
use pns_domain::recap::git_block::PullRequestLookup;

#[test]
fn a_listing_with_one_pull_request_reads_its_number_and_state() {
    let PullRequestLookup::Found(request) = listed(
        "count: 1 of 1 total\n\
         pull_requests[1]{number,title,state,author,draft,review}:\n  \
         586,\"feat(openspec): track the global OpenSpec configuration\",merged,webdavis,no,none\n\
         help[2]:\n  Run `gh-axi pr view <number>` to view details\n",
    ) else {
        panic!("the listing was not read");
    };
    assert_eq!(request.number, 586);
    assert_eq!(request.state, "merged");
}

#[test]
fn a_title_holding_commas_does_not_move_the_state_column() {
    // THE ONE FIELD THAT MAY HOLD A COMMA is the title, which is why the state
    // is counted from the right of the row.
    let PullRequestLookup::Found(request) = listed(
        "pull_requests[1]{number,title,state,author,draft,review}:\n  \
         591,\"feat(pns): recap agent, recap git, and the skill\",open,webdavis,no,none\n",
    ) else {
        panic!("the listing was not read");
    };
    assert_eq!((request.number, request.state.as_str()), (591, "open"));
}

#[test]
fn an_empty_listing_is_no_pull_request_rather_than_no_answer() {
    assert_eq!(
        listed("count: 0\npull_requests: []\nhelp[2]:\n"),
        PullRequestLookup::Absent
    );
}

#[test]
fn output_that_is_not_a_listing_at_all_is_unavailable() {
    // FAIL CLOSED INTO "NOTHING ANSWERED", never into "there is no pull
    // request": a gh-axi that printed an error must not become a recap line
    // saying the branch has no PR.
    for refused in [
        "error: \"not authenticated\"\ncode: AUTH_ERROR\n",
        "",
        "pull_requests[1]{number,title,state,author,draft,review}:\n",
    ] {
        assert_eq!(
            listed(refused),
            PullRequestLookup::Unavailable,
            "{refused:?}"
        );
    }
}
