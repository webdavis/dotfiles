//! gh-axi's own pull-request listing format, read.

use pns_domain::recap::git_block::{PullRequest, PullRequestLookup};

/// One pull request off a gh-axi listing, or which of the two absences it was.
///
/// THE FIELD NAMES ARE IN THE LISTING and the state is counted from the RIGHT
/// of the row, because the title is the one field that may hold a comma and it
/// sits to the left of everything else. gh-axi has no JSON mode (MEASURED
/// 2026-09-14: `--format` is not a flag it knows), so this reads the format it
/// prints.
pub(super) fn listed(listing: &str) -> PullRequestLookup {
    let Some(header) = listing.lines().find(|line| line.starts_with(PULL_REQUESTS)) else {
        return PullRequestLookup::Unavailable;
    };
    // `pull_requests: []` is gh-axi saying this branch has none, which is the
    // one absence the layout has a word for.
    let Some(fields) = header
        .split_once('{')
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(names, _)| names.split(',').collect::<Vec<_>>())
    else {
        return PullRequestLookup::Absent;
    };
    let Some(row) = listing
        .lines()
        .skip_while(|line| !line.starts_with(PULL_REQUESTS))
        .nth(1)
    else {
        return PullRequestLookup::Unavailable;
    };
    // THE NUMBER MUST BE THE FIRST COLUMN. It is the receipt, and a listing
    // shaped some other way is not the one this was written against.
    let (Some(&"number"), Some(state)) = (fields.first(), column(row, &fields, "state")) else {
        return PullRequestLookup::Unavailable;
    };
    let Some(number) = row
        .trim_start()
        .split(',')
        .next()
        .and_then(|number| number.parse().ok())
    else {
        return PullRequestLookup::Unavailable;
    };
    PullRequestLookup::Found(PullRequest { number, state })
}

/// One named column of a listing row, counted from the right. See `listed`.
fn column(row: &str, fields: &[&str], name: &str) -> Option<String> {
    let from_end = fields.len() - 1 - fields.iter().position(|field| *field == name)?;
    Some(row.rsplit(',').nth(from_end)?.trim().to_string())
}

/// The line gh-axi's pull-request listing opens with.
const PULL_REQUESTS: &str = "pull_requests";

#[cfg(test)]
#[path = "listing/tests.rs"]
mod tests;
