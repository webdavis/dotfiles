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
