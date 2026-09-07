//! The one sentence the record carries about a subject, and the quoting that
//! keeps third-party text from rendering as something the operator never
//! wrote.

use super::changes::{Listing, State, tuples};

/// How many changed names the sentence lists before it counts the rest.
const NAME_CAP: usize = 12;

/// One section of the record: what moved, or WHY NOTHING HERE SAYS SO.
///
/// A reading that failed must never render as "0 of 0 tracked entries
/// changed", which is what a quiet week looks like. A run that could not ask
/// what is installed knows nothing about the week, and the sentence has to say
/// which of the two it is.
pub fn change_section(
    before: &Result<Listing, String>,
    after: &Result<Listing, String>,
    label: &str,
    caveat: &str,
    source: &str,
) -> String {
    let (Ok(before), Ok(after)) = (before, after) else {
        return format!(
            "{label}: NOT COMPARED, {source} failed on this run, so nothing here says what \
             changed; it says this run could not read what is installed."
        );
    };
    change_line(before, after, label, caveat)
}

/// THE TOTAL IS THE TRACKED POPULATION, which the tuples deliberately do not
/// describe: they list only what moved. It is the after-rows plus the
/// removals, because counting the after-rows alone renders the impossible
/// "2 of 0 tracked entries changed" on an emptied listing.
///
/// `caveat` is what this subject CANNOT tell you, restated on every entry
/// rather than assumed known: a record implying a completeness it does not
/// have is worse than no record.
fn change_line(before: &Listing, after: &Listing, label: &str, caveat: &str) -> String {
    let mut total = after.len();
    let mut moved = Vec::new();
    for entry in tuples(before, after) {
        match entry.state {
            State::Added => moved.push(format!("{} (added)", code(&entry.name))),
            // A REMOVAL IS THE MOST WORTH-SEEING LINE HERE: something left
            // without being asked to.
            State::Removed => {
                total += 1;
                moved.push(format!("{} (removed)", code(&entry.name)));
            }
            State::Changed => moved.push(format!(
                "{} {} -> {}",
                code(&entry.name),
                code(&entry.before),
                code(&entry.after)
            )),
        }
    }
    if moved.is_empty() {
        return format!("{label}: 0 of {total} tracked entries changed. {caveat}");
    }
    let mut shown = moved[..moved.len().min(NAME_CAP)].join(", ");
    if moved.len() > NAME_CAP {
        shown = format!("{shown}, and {} more", moved.len() - NAME_CAP);
    }
    format!(
        "{label}: {} of {total} tracked entries changed ({shown}). {caveat}",
        moved.len()
    )
}

/// Third-party text as a Discord inline code span.
///
/// Every name and version here is chosen by whoever published the package, and
/// it lands in a channel whose whole value is that its contents read as
/// trustworthy machine records. Unquoted, a version of
/// `[urgent: click here](https://evil.example)` renders as a CLICKABLE LINK
/// the operator never authored. The two things that could close the span
/// early, a backtick and any control character, are removed first.
pub fn code(text: &str) -> String {
    let quoted: String = text
        .chars()
        .filter(|letter| *letter != '`' && !letter.is_control())
        .collect();
    format!("`{quoted}`")
}

#[cfg(test)]
mod tests {
    use super::super::changes::tests::listing;
    use super::*;

    fn section(before: &Listing, after: &Listing) -> String {
        change_section(
            &Ok(before.clone()),
            &Ok(after.clone()),
            "formulae and casks",
            "the caveat",
            "unused",
        )
    }

    #[test]
    fn a_reading_that_failed_says_so_rather_than_reading_as_a_quiet_week() {
        let said = change_section(
            &Ok(listing(&[])),
            &Err("exit 1".to_string()),
            "formulae and casks",
            "the caveat",
            "brew list --versions",
        );
        assert!(said.contains("NOT COMPARED"), "{said}");
        assert!(said.contains("brew list --versions"), "{said}");
        assert!(!said.contains("0 of 0"), "{said}");
    }

    #[test]
    fn a_week_that_moved_nothing_still_states_what_was_tracked_and_the_caveat() {
        let same = listing(&[("jq", "1.7.1"), ("just", "1.36.0")]);
        assert_eq!(
            section(&same, &same),
            "formulae and casks: 0 of 2 tracked entries changed. the caveat"
        );
    }

    #[test]
    fn a_removal_counts_toward_the_total_it_is_no_longer_part_of() {
        // Counting the after-rows alone renders "1 of 0 tracked entries
        // changed" on an emptied listing, which is impossible on its face.
        let said = section(&listing(&[("gone", "1.0")]), &listing(&[]));
        assert!(said.contains("1 of 1 tracked entries"), "{said}");
        assert!(said.contains("(removed)"), "{said}");
    }

    #[test]
    fn an_addition_is_named_as_one_and_counted_in_the_population_it_joined() {
        let said = section(&listing(&[]), &listing(&[("new", "0.1")]));
        assert!(said.contains("1 of 1 tracked entries"), "{said}");
        assert!(said.contains("`new` (added)"), "{said}");
    }

    #[test]
    fn a_version_transition_is_rendered_with_both_versions() {
        let said = section(&listing(&[("jq", "1.7.0")]), &listing(&[("jq", "1.7.1")]));
        assert!(said.contains("`jq` `1.7.0` -> `1.7.1`"), "{said}");
    }

    #[test]
    fn a_week_that_moved_more_names_than_fit_lists_the_cap_and_counts_the_rest() {
        let before: Listing = (0..NAME_CAP + 3)
            .map(|number| (format!("formula-{number:02}"), "1.0".to_string()))
            .collect();
        let said = section(&before, &listing(&[]));
        assert!(said.contains("and 3 more"), "{said}");
        assert!(said.contains("formula-11"), "{said}");
        assert!(!said.contains("formula-12"), "{said}");
    }

    #[test]
    fn a_published_version_cannot_close_its_own_code_span_or_render_as_a_link() {
        let quoted = code("`[click](https://evil.example)`\u{7}");
        assert_eq!(quoted, "`[click](https://evil.example)`");
        assert_eq!(quoted.matches('`').count(), 2);
    }

    #[test]
    fn every_name_and_version_in_a_section_is_quoted() {
        let said = section(&listing(&[("ok", "1.0")]), &listing(&[("ok", "`x`")]));
        assert!(said.contains("`ok` `1.0` -> `x`"), "{said}");
    }
}
