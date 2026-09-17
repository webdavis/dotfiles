use super::*;
use posture_application::AlertSignal;

fn alert(occurrence: Option<&str>) -> Alert {
    Alert {
        occurrence_id: occurrence.map(str::to_string),
        event: "alert",
        signal: AlertSignal::NeedsAttention,
        severity: None,
        occurred_at: None,
        title: "title".into(),
        detail: "detail".into(),
    }
}

#[test]
fn one_seed_always_derives_the_one_id_the_producer_envelope_already_carries() {
    // The exact value the producer's own request test pins, so the lifted
    // derivation cannot drift from the id already on the wire.
    assert_eq!(
        derive("occurrence-7"),
        "posture-d28d5af268c004d795ce0240f35f5218"
    );
    assert_eq!(derive("occurrence-7"), derive("occurrence-7"));
    assert_ne!(derive("occurrence-7"), derive("occurrence-8"));
}

#[test]
fn a_page_with_an_occurrence_seeds_from_it_and_one_without_never_collides() {
    assert_eq!(seed(&alert(Some("occurrence-7"))), "occurrence-7");
    assert_ne!(seed(&alert(None)), seed(&alert(None)));
}

#[test]
fn a_copy_takes_an_id_of_its_own_that_is_still_derived_from_the_pages() {
    let page = derive("occurrence-7");
    assert_ne!(derive_copy(&page), page);
    assert_eq!(derive_copy(&page), derive_copy(&page));
    assert!(derive_copy(&page).starts_with("posture-"));
    // Two pages' copies are as distinct as the pages themselves.
    assert_ne!(derive_copy(&page), derive_copy(&derive("occurrence-8")));
}
