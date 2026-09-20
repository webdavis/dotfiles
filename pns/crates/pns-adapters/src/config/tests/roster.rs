use super::*;

#[test]
fn every_key_the_roster_declares_is_read_by_the_table_that_declares_it() {
    // THE ROSTER IS THE SCHEMA'S ONE STATEMENT and this is what stops it
    // becoming a second, drifting one. A key declared with no arm to read
    // it is refused by that arm, which is a table whose refusal names a
    // key it will not accept; a key an arm reads that the roster does not
    // declare stops working, because the roster is checked first. Both are
    // red, and this walk is the half that catches the first.
    let mut walked: Vec<(&str, &str)> = Vec::new();
    for (table, key, value) in SAMPLE_VALUES.iter().copied() {
        let text = config_writing(table, key, value);
        assert!(
            parse_config(&text).is_ok(),
            "{} declares `{key}` and will not parse it: {:?}",
            shown_as(table),
            parse_config(&text)
        );
        walked.push((table, key));
    }

    // AND THE TWO SETS ARE THE SAME SET, or the walk above proves only
    // whatever half of the roster someone remembered to sample.
    let mut declared: Vec<(&str, &str)> = super::super::TABLE_KEYS
        .iter()
        .flat_map(|(table, keys)| keys.iter().map(move |key| (*table, *key)))
        .collect();
    walked.sort_unstable();
    declared.sort_unstable();
    assert_eq!(
        walked, declared,
        "every declared key is walked, and no more"
    );
}

/// The three level words `lights.<level>` stands for, each of them a table
/// keyed by the operator's own lamp, room or zone name. They are asserted to
/// be keys of `lights` below, so a level renamed in the roster and not here
/// is red rather than unjudged.
const TARGET_LEVELS: [&str; 3] = ["lamp", "room", "zone"];

/// A name is plural when it ends in an `s` that is not the `ss` of a
/// singular word (`delivery_class`), which is the whole test this vocabulary
/// needs: every table name in the roster is one ASCII word or an underscored
/// pair of them.
fn is_plural(name: &str) -> bool {
    name.ends_with('s') && !name.ends_with("ss")
}

/// The last segment of a roster row, which is the name an operator writes.
fn last_segment(table: &str) -> &str {
    table.rsplit('.').next().expect("a row has a segment")
}

/// The tables the commit message names as the rule's other side: not `open`
/// in the schema's sense (their keys are declared, not the operator's own),
/// but each one holds more than a single setting, so its own name is plural
/// too. Named here rather than derived, so a rename to singular is red even
/// though nothing about the table stops being closed.
const CLOSED_SET_HOLDERS: [&str; 5] = ["routes", "plugins", "paths", "lights", "failures"];

#[test]
fn the_plural_rule_holds_one_way_round_across_the_whole_roster() {
    for (table, _) in super::super::TABLE_KEYS.iter().copied() {
        // A TABLE HOLDING A SET IS PLURAL. The open tables are the
        // set-holders the schema itself marks, so `is_open` is what this
        // reads rather than a second list that could disagree with it.
        if super::super::schema::is_open(table) {
            assert!(
                is_plural(last_segment(table)),
                "`{table}` takes the operator's own keys and is singular"
            );
        }
        // AND A TABLE KEYED BY ONE NAME IS SINGULAR. Those rows are the ones
        // whose last segment is a placeholder for the operator's own name, so
        // the segment the operator actually writes is the one before it.
        if let Some(keyed) = table
            .strip_suffix("<name>")
            .map(|head| head.trim_end_matches('.'))
        {
            assert!(
                !is_plural(last_segment(keyed)),
                "`{keyed}` is keyed by one name and is plural"
            );
        }
    }

    // THE CLOSED SET-HOLDERS ARE THE OTHER HALF OF THE RULE: `is_open` never
    // sees them, so nothing above this line walks them.
    for table in CLOSED_SET_HOLDERS {
        assert!(is_plural(table), "`{table}` holds a set and is singular");
    }

    // THE THREE LEVELS ARE THE SAME SHAPE one row further in: their own row
    // is a prefix, so the names they are keyed by never appear in it.
    let lights = super::super::keys_of("lights").expect("`lights` is a roster row");
    for level in TARGET_LEVELS {
        assert!(lights.contains(&level), "`lights` has no `{level}` key");
        assert!(
            !is_plural(level),
            "`lights.{level}` is keyed by one name and is plural"
        );
    }
}
