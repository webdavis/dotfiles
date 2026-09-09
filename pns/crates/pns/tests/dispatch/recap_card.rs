use super::*;

#[test]
fn the_recap_card_is_exactly_what_the_entries_compose_and_nothing_a_model_said() {
    // GUARD, and it is green by design. PR 2 has no summarizer at all, so this
    // states the body a mechanical composition produces, in full, as the thing
    // a later slice's model output must never be allowed to replace. Its teeth
    // arrive with the summarizer: the same assertion is what catches an
    // implementer splicing a model's answer into the phone card, which is the
    // one layer the locked spec says the model never touches.
    let sandbox = Sandbox::new("recap-card-mechanical");
    record_every_event(&sandbox);
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and one recap card: {raised:?}"
    );
    assert_eq!(
        raised[1]["detail"], "claude · blocked · p4. 13 events, 2 missed. recap in #pns",
        "the card is composed, never summarized: {raised:?}"
    );
}
