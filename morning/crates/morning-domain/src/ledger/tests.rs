use super::*;

/// A ledger shaped like this repository's own: open items, a closed one, a
/// continuation line carrying the phrase that classifies its item.
const LEDGER: &str = "\
# Remaining work

## Deployment

- [ ] 12. Ship the posture converge. Deployed on the next full run;
  the operator owes a `chezmoi apply` once it merges.
- [x] 13. Already done, and it mentions chezmoi apply and the operator.
- [ ] 14. Rename the uu lanes. No deployment step.
- [ ] 15. Pair the phone again, which only the operator can do.
";

fn markers() -> (Vec<String>, Vec<String>) {
    (
        vec!["chezmoi apply".to_string()],
        vec!["operator".to_string()],
    )
}

#[test]
fn finds_the_owed_apply_from_a_continuation_line() {
    let (applies, operators) = markers();
    assert_eq!(
        parse(LEDGER, &applies, &operators).owed_applies,
        vec!["12. Ship the posture converge. Deployed on the next full run;".to_string()]
    );
}

#[test]
fn finds_every_operator_owned_item_and_no_closed_one() {
    let (applies, operators) = markers();
    let found = parse(LEDGER, &applies, &operators);
    assert_eq!(found.operator_items.len(), 2);
    assert!(found.operator_items[1].starts_with("15. Pair the phone"));
    assert!(
        !found
            .operator_items
            .iter()
            .any(|item| item.starts_with("13."))
    );
}

#[test]
fn an_item_with_neither_phrase_is_reported_nowhere() {
    let (applies, operators) = markers();
    let found = parse(LEDGER, &applies, &operators);
    assert!(
        !found
            .owed_applies
            .iter()
            .any(|item| item.starts_with("14."))
    );
    assert!(
        !found
            .operator_items
            .iter()
            .any(|item| item.starts_with("14."))
    );
}

#[test]
fn a_ledger_with_no_open_items_classifies_nothing() {
    let (applies, operators) = markers();
    assert_eq!(
        parse(
            "- [x] done, operator, chezmoi apply\n",
            &applies,
            &operators
        ),
        Ledger::default()
    );
}

#[test]
fn unindented_prose_after_a_blank_line_does_not_join_the_item_above() {
    let (applies, operators) = markers();
    let ledger = "\
- [ ] 1. Rename the widget.

Unrelated prose: the operator must run `chezmoi apply` after the unrelated task below.

- [ ] 2. Other thing.
";
    let found = parse(ledger, &applies, &operators);
    assert!(found.owed_applies.is_empty());
    assert!(found.operator_items.is_empty());
}

#[test]
fn a_phrase_matches_whatever_case_the_ledger_wrote_it_in() {
    let (_, operators) = markers();
    let found = parse("- [ ] 9. The Operator confirms it.\n", &[], &operators);
    assert_eq!(
        found.operator_items,
        vec!["9. The Operator confirms it.".to_string()]
    );
}
