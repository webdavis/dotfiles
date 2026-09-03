//! Reading what is installed, and what moved between two readings.
//!
//! TWO CONSUMERS, ONE WALK. The record says what a week upgraded; the osquery
//! file-integrity page asks, days later, whether a recorded upgrade plausibly
//! explains a file whose hash left its known-good manifest. Both readings come
//! from `tuples`, so the channel and the page can never disagree about what a
//! week did. The sentence itself lives next door in `sections`.

/// One reading of what is installed: `(name, fingerprint)` pairs, sorted by
/// name. The fingerprint is a version both subjects report honestly.
pub type Listing = Vec<(String, String)>;

/// How one name moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Added,
    Removed,
    Changed,
}

/// One name that moved, with both sides of the transition. The absent side of
/// an add or a remove is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub name: String,
    pub state: State,
    pub before: String,
    pub after: String,
}

/// `brew list --versions` prints `<name> <version> [<version>...]`. The
/// remainder is JOINED rather than truncated, so a formula keeping two
/// versions installed reads as one fingerprint instead of losing the second.
pub fn parse_brew_versions(stdout: &str) -> Listing {
    let mut rows: Listing = stdout
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?;
            let versions: Vec<&str> = words.collect();
            if versions.is_empty() {
                return None;
            }
            Some((name.to_string(), versions.join(" ")))
        })
        .collect();
    rows.sort();
    rows
}

/// `mas list` prints `<id> <Name> (<version>)`. The id is the stable key but
/// the NAME is what a reader recognizes, so the name is the key here and the
/// id is dropped. A line that is not that shape is skipped rather than
/// half-parsed: an empty App Store is a truthful nothing, not a failure.
pub fn parse_mas_list(stdout: &str) -> Listing {
    let mut rows: Listing = stdout
        .lines()
        .filter_map(|line| {
            let head = line.trim_end().strip_suffix(')')?;
            let open = head.rfind('(')?;
            let version = head[open + 1..].trim();
            let (id, name) = head[..open].trim().split_once(char::is_whitespace)?;
            if version.is_empty() || !id.chars().all(|digit| digit.is_ascii_digit()) {
                return None;
            }
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_string(), version.to_string()))
        })
        .collect();
    rows.sort();
    rows
}

/// Every name that MOVED between two readings. A pair with nothing between
/// them answers with nothing at all.
pub fn tuples(before: &Listing, after: &Listing) -> Vec<Change> {
    let mut moved = Vec::new();
    for (name, fingerprint) in after {
        match look_up(before, name) {
            None => moved.push(change(name, State::Added, "", fingerprint)),
            Some(was) if was != fingerprint => {
                moved.push(change(name, State::Changed, was, fingerprint))
            }
            Some(_) => {}
        }
    }
    for (name, fingerprint) in before {
        if look_up(after, name).is_none() {
            moved.push(change(name, State::Removed, fingerprint, ""));
        }
    }
    moved
}

/// ponytail: a linear scan per name, so the walk is quadratic. A few hundred
/// formulae is microseconds; sort-merge or a map if a listing ever runs to
/// tens of thousands.
fn look_up<'a>(listing: &'a Listing, wanted: &str) -> Option<&'a str> {
    listing
        .iter()
        .find(|(name, _)| name == wanted)
        .map(|(_, fingerprint)| fingerprint.as_str())
}

pub(crate) fn change(name: &str, state: State, before: &str, after: &str) -> Change {
    Change {
        name: name.to_string(),
        state,
        before: before.to_string(),
        after: after.to_string(),
    }
}

/// One tuple as the tab-separated row the upgrade record persists:
/// `<name>\t<added|removed|changed>\t<before>\t<after>`.
///
/// A TAB INSIDE ANY FIELD IS DELETED, because the row is read back field by
/// field and one would shift a version into the state's column.
pub fn tuple_row(moved: &Change) -> String {
    let state = match moved.state {
        State::Added => "added",
        State::Removed => "removed",
        State::Changed => "changed",
    };
    format!(
        "{}\t{state}\t{}\t{}",
        untabbed(&moved.name),
        untabbed(&moved.before),
        untabbed(&moved.after)
    )
}

fn untabbed(field: &str) -> String {
    field.replace('\t', "")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn listing(rows: &[(&str, &str)]) -> Listing {
        rows.iter()
            .map(|(name, fingerprint)| (name.to_string(), fingerprint.to_string()))
            .collect()
    }

    #[test]
    fn a_formula_keeping_two_versions_installed_reads_as_one_fingerprint() {
        assert_eq!(
            parse_brew_versions("jq 1.7.1\nopenssl@3 3.4.0 3.3.2\n"),
            listing(&[("jq", "1.7.1"), ("openssl@3", "3.4.0 3.3.2")])
        );
    }

    #[test]
    fn a_listing_line_with_no_version_is_not_an_entry() {
        assert_eq!(parse_brew_versions("jq\n\n  \n"), listing(&[]));
    }

    #[test]
    fn app_store_apps_are_keyed_by_the_name_a_reader_recognizes_not_the_id() {
        assert_eq!(
            parse_mas_list("497799835 Xcode (16.2)\n1444383602 GoodNotes 5 (6.1.5)\n"),
            listing(&[("GoodNotes 5", "6.1.5"), ("Xcode", "16.2")])
        );
    }

    #[test]
    fn a_line_the_app_store_did_not_shape_that_way_is_skipped_rather_than_half_parsed() {
        for line in ["No installed apps found", "497799835 Xcode", "Xcode (16.2)"] {
            assert_eq!(parse_mas_list(line), listing(&[]), "{line}");
        }
    }

    #[test]
    fn every_name_that_moved_is_reported_with_both_sides_of_its_transition() {
        let moved = tuples(
            &listing(&[("gone", "1.0"), ("kept", "2.0"), ("moved", "3.0")]),
            &listing(&[("kept", "2.0"), ("moved", "3.1"), ("new", "0.1")]),
        );
        assert_eq!(
            moved,
            vec![
                change("moved", State::Changed, "3.0", "3.1"),
                change("new", State::Added, "", "0.1"),
                change("gone", State::Removed, "1.0", ""),
            ]
        );
    }

    #[test]
    fn two_readings_with_nothing_between_them_report_nothing_at_all() {
        let same = listing(&[("jq", "1.7.1")]);
        assert_eq!(tuples(&same, &same), vec![]);
    }

    #[test]
    fn a_tab_inside_a_field_is_deleted_so_a_row_cannot_shift_its_own_columns() {
        let row = tuple_row(&change("na\tme", State::Changed, "1\t0", "2\t0"));
        assert_eq!(row, "name\tchanged\t10\t20");
        assert_eq!(row.split('\t').count(), 4);
    }
}
