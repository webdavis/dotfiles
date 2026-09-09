use super::*;
mod fixture;
use fixture::{World, configured};

#[test]
fn all_three_recap_sections_spend_one_budget_started_after_both_external_reads() {
    let world = World::one();
    let body = world.build(&configured());
    assert_eq!(
        *world.log.borrow(),
        [
            "activity",
            "merges",
            "notes",
            "budget",
            "summary(6)",
            "summary(2)",
            "summary(0)"
        ]
    );
    assert!(body.contains("selected line"));
    assert!(
        body.contains("finding.md"),
        "the exhausted note section must retain its mechanical source"
    );
}

#[test]
fn an_empty_unconfigured_recap_reads_no_external_source_and_starts_no_summarizer() {
    let world = World {
        entries: Vec::new(),
        ..World::one()
    };
    let body = world.build(&Recap {
        summarizer: configured().summarizer,
        ..Recap::default()
    });
    assert_eq!(*world.log.borrow(), ["activity", "budget"]);
    assert!(body.contains("nothing was recorded"));
}

#[test]
fn unavailable_recap_sources_never_spend_a_summary_call_for_the_missing_section() {
    let world = World {
        available: false,
        ..World::one()
    };
    let body = world.build(&configured());
    assert_eq!(
        *world.log.borrow(),
        ["activity", "merges", "notes", "budget", "summary(6)"]
    );
    assert!(!body.contains("merge title"));
    assert!(!body.contains("finding.md"));
}

#[test]
fn a_recap_with_unreadable_local_time_posts_the_same_placeholder_in_header_and_timeline() {
    let world = World::one();
    let body = world.build(&Recap::default());
    let sent = std::cell::RefCell::new(Vec::new());
    assert_eq!(
        crate::post_return_recap(&body, false, |body, route| {
            sent.borrow_mut()
                .push((body.to_string(), route.to_string()));
            vec![pns_domain::Delivery::Silent]
        }),
        0
    );
    let sent = sent.borrow();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "");
    assert_eq!(sent[0].0.matches("--:--").count(), 3);
    assert!(sent[0].0.contains("finished"));
}
