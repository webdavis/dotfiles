use super::*;
mod fixture;
use fixture::{World, configured};

#[test]
fn every_source_is_read_once_before_the_one_summarizer_budget_starts() {
    // ONE EPISODE, ONE BUDGET: the reads happen first, then the deadline is
    // taken, so a slow model cannot cost a source its own window.
    // `pull_requests` is asked twice, once for its own windowed section and
    // once with no window at all for `open`, which is the design's own
    // reading of a listing that has no window.
    let world = World::one();
    let body = world.build(&configured());
    assert_eq!(
        *world.log.borrow(),
        [
            "activity",
            "run(gh Some(100) Some(200))",
            "notes",
            "run(gh None None)",
            "budget",
            "summary(6)",
            "summary(2)",
        ]
    );
    assert!(body.contains("selected line"), "{body}");
    assert!(
        body.contains("finding.md"),
        "the exhausted note section must retain its mechanical source: {body}"
    );
}

#[test]
fn an_empty_unconfigured_recap_reads_no_source_and_starts_no_summarizer() {
    let world = World {
        events: Vec::new(),
        ..World::one()
    };
    let body = world.build(&Recap {
        summarizer: configured().summarizer,
        ..Recap::default()
    });
    assert_eq!(*world.log.borrow(), ["activity", "budget"]);
    assert!(body.contains("nothing was recorded"), "{body}");
}

#[test]
fn an_unavailable_source_never_spends_a_summary_call_for_the_missing_section() {
    let world = World {
        available: false,
        ..World::one()
    };
    let body = world.build(&configured());
    assert_eq!(
        *world.log.borrow(),
        [
            "activity",
            "run(gh Some(100) Some(200))",
            "notes",
            "run(gh None None)",
            "budget",
            "summary(6)",
        ]
    );
    assert!(!body.contains("merge title"), "{body}");
    assert!(!body.contains("finding.md"), "{body}");
}

#[test]
fn a_section_nobody_named_is_never_gathered_at_all() {
    // `--section` NARROWS THE WORK, not only the output: a page that shows
    // no tasks must not spawn the task tool to find that out.
    let world = World::one();
    let recap = configured();
    world.assemble(&recap, vec!["agents".to_string()]);
    assert_eq!(*world.log.borrow(), ["activity", "budget", "summary(6)"]);
}

#[test]
fn a_recap_with_unreadable_local_time_posts_the_same_placeholder_in_header_and_timeline() {
    let world = World::one();
    let body = world.build(&Recap::default());
    let sent = std::cell::RefCell::new(Vec::new());
    assert_eq!(
        crate::post_return_recap(&body, |body, route| {
            sent.borrow_mut()
                .push((body.to_string(), route.to_string()));
            vec![pns_domain::Delivery::Silent]
        }),
        0
    );
    let sent = sent.borrow();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "");
    assert_eq!(sent[0].0.matches("--:--").count(), 2);
    assert!(sent[0].0.contains("finished"), "{}", sent[0].0);
}

#[test]
fn the_document_carries_every_configured_section_and_the_windows_own_bounds() {
    let world = World::one();
    let recap = configured();
    let assembled = world.assemble(&recap, Vec::new());
    let node = document(&assembled, |at| format!("t{at}"));
    let written = format!("{node:?}");
    for field in [
        "schema",
        "window",
        "sections",
        "agents",
        "pull_requests",
        "review_notes",
        "open",
    ] {
        assert!(written.contains(field), "{field} is missing: {written}");
    }
    assert!(
        written.contains("t100") && written.contains("t200"),
        "{written}"
    );
}

/// The delivered page, byte for byte.
///
/// A GOLDEN FIXTURE, because this is what the operator reads on every return.
/// The engine now renders the return card's body as well as a typed
/// `pns recap`, so a change to either is a change to both; a snapshot is what
/// makes that visible in a diff rather than at the desk the next morning.
const PAGE_GOLDEN: &str = include_str!("tests/recap-page.golden");

/// The document, schema 1, byte for byte. ONE FIXTURE PER SCHEMA VERSION: a
/// consumer checks `schema`, so a change to the shape bumps the number and
/// earns a fixture of its own rather than editing this one.
const DOCUMENT_GOLDEN: &str = include_str!("tests/recap-document-schema-1.golden");

/// The page and the document over one fixed window, for the two goldens.
fn golden_world() -> (World, Recap) {
    (World::one(), configured())
}

#[test]
fn the_delivered_page_is_the_golden_fixture_byte_for_byte() {
    // `PNS_WRITE_GOLDEN=1` REWRITES IT, because a golden edited by hand to
    // make a test pass is a golden that pins nothing.
    let (world, recap) = golden_world();
    let page = world.build(&recap);
    if std::env::var_os("PNS_WRITE_GOLDEN").is_some() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/build_recap/tests/recap-page.golden"
            ),
            format!("{page}\n"),
        )
        .expect("the golden");
    }
    assert_eq!(page, PAGE_GOLDEN.trim_end_matches('\n'));
}

#[test]
fn the_document_is_the_golden_fixture_for_its_own_schema_version() {
    let (world, recap) = golden_world();
    let assembled = world.assemble(&recap, Vec::new());
    let node = document(&assembled, |at| format!("2026-09-19T00:00:{at:02}-04:00"));
    if std::env::var_os("PNS_WRITE_GOLDEN").is_some() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/build_recap/tests/recap-document-schema-1.golden"
            ),
            format!("{node:#?}\n"),
        )
        .expect("the golden");
    }
    assert_eq!(
        format!("{node:#?}\n"),
        DOCUMENT_GOLDEN,
        "the document's shape moved without its schema version moving with it"
    );
}

#[test]
fn the_documents_open_section_always_carries_the_dead_letter_count() {
    // ALWAYS PRESENT, zero included: the document carries the whole shape
    // every time, so this is additive and the schema version does not move.
    let written = |world: &World| {
        format!(
            "{:?}",
            document(&world.assemble(&configured(), Vec::new()), |at| format!(
                "t{at}"
            ))
        )
    };
    assert!(
        written(&World::one()).contains(r#"("dead_lettered", Number(0))"#),
        "{}",
        written(&World::one())
    );
    let some = World {
        dead_lettered: 3,
        ..World::one()
    };
    assert!(
        written(&some).contains(r#"("dead_lettered", Number(3))"#),
        "{}",
        written(&some)
    );
}

#[test]
fn the_page_reports_the_dead_letter_count_the_store_holds() {
    let world = World {
        dead_lettered: 2,
        ..World::one()
    };
    let page = world.build(&configured());
    assert!(
        page.contains("- 2 legs dead-lettered, run pns failures"),
        "{page}"
    );
}
