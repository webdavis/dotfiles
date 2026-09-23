use super::*;

const NOW: u64 = 1_790_133_900; // 2026-09-23T03:25:00Z

/// The index names the one page this site serves today, links it, and
/// describes it in a muted line.
#[test]
fn the_index_lists_failures_linked_with_a_muted_description() {
    let rendered = index_page(3, NOW);
    assert!(rendered.contains("<h2>pns</h2>"), "{rendered}");
    assert!(
        rendered.contains("href=\"/failures\">Failures</a>"),
        "{rendered}"
    );
}

/// The live count sits beside the row: a number when something is failing,
/// "none" when nothing is.
#[test]
fn the_live_count_reads_none_at_zero_and_the_number_otherwise() {
    assert!(index_page(0, NOW).contains(">none<"));
    assert!(!index_page(0, NOW).contains(">0<"));
    assert!(index_page(3, NOW).contains(">3<"));
}

/// The index is dark always, the same as every other page here.
#[test]
fn the_index_is_dark_only() {
    let rendered = index_page(0, NOW);
    assert!(
        rendered.contains("<meta name=\"color-scheme\" content=\"dark\">"),
        "{rendered}"
    );
    assert!(!rendered.contains("light-dark("), "{rendered}");
    assert!(!rendered.contains("prefers-color-scheme"), "{rendered}");
}

/// The footer is the same one every card carries.
#[test]
fn the_index_carries_the_shared_footer() {
    let rendered = index_page(0, NOW);
    assert!(rendered.contains("All times UTC"), "{rendered}");
    assert!(
        rendered.contains("Updated September 23, 2026 at 03:25"),
        "{rendered}"
    );
}
