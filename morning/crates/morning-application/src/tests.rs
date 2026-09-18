use super::*;

fn body(section: &Section) -> String {
    match &section.body {
        SectionBody::Lines(lines) => lines.join("|"),
        SectionBody::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

#[test]
fn every_source_gets_a_section_in_the_order_it_was_given() {
    let sections = gather(vec![
        Source::new("First", || SourceOutcome::Lines(vec!["a".into()])),
        Source::new("Second", || {
            SourceOutcome::Unavailable("not configured".into())
        }),
    ]);
    let titles: Vec<_> = sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["First", "Second"]);
    assert_eq!(body(&sections[1]), "unavailable: not configured");
}

#[test]
fn sources_are_read_at_the_same_time_rather_than_one_after_another() {
    let running = std::sync::atomic::AtomicUsize::new(0);
    let peak = std::sync::atomic::AtomicUsize::new(0);
    let watch = || {
        let now = running.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        peak.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
        while peak.load(std::sync::atomic::Ordering::SeqCst) < 3 {
            std::hint::spin_loop();
        }
        SourceOutcome::Lines(vec!["seen".into()])
    };
    gather(vec![
        Source::new("One", watch),
        Source::new("Two", watch),
        Source::new("Three", watch),
    ]);
    assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 3);
}

#[test]
fn a_source_that_panics_becomes_an_unavailable_section() {
    let sections = gather(vec![Source::new("Boom", || panic!("no"))]);
    assert_eq!(body(&sections[0]), "unavailable: the reader panicked");
}
