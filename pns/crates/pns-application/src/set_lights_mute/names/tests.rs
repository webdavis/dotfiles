use super::asks_the_bridge;

#[test]
fn only_a_word_no_declaration_accounts_for_is_worth_a_bridge_listing() {
    // THE MUTE'S VOCABULARY IS BOTH SOURCES, and the bridge half costs a
    // human three round trips while they stand at a terminal. A place the
    // config already declares can be enforced whatever the bridge says, so
    // the ordinary bedtime mute must not pay for a listing that cannot
    // change the answer.
    let declared = vec!["3F - Studio".to_string()];
    let typed =
        |words: &[&str]| -> Vec<String> { words.iter().map(|word| (*word).to_string()).collect() };
    assert!(!asks_the_bridge(&declared, &typed(&[])), "the bare report");
    assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio"])));
    assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio", "2h"])));
    assert!(
        !asks_the_bridge(&declared, &typed(&["3F - Nowhere", "off"])),
        "`off` is allowed over any name, so no listing could change it"
    );
    // AND THE ONE CASE A LISTING DECIDES: a name no declaration holds may
    // still be a real lamp, room or zone, which is the whole grammar.
    assert!(asks_the_bridge(&declared, &typed(&["3F - Studio - HCL1"])));
    assert!(asks_the_bridge(
        &declared,
        &typed(&["3F - Studio - HCL1", "2h"])
    ));
}
