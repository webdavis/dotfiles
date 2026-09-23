use super::*;

/// The footer names the render time, never the operator's mockup
/// placeholder.
#[test]
fn the_footer_names_the_render_time_and_never_says_sample_data() {
    let rendered = footer(1_790_133_900); // 2026-09-23T03:25:00Z
    assert!(rendered.contains("All times UTC"), "{rendered}");
    assert!(
        rendered.contains("Updated September 23, 2026 at 03:25"),
        "{rendered}"
    );
    assert!(!rendered.contains("Sample data"), "{rendered}");
}

/// The relative "in N minutes" a due time is read as, rounded up so a
/// deadline less than a minute out still reads as one rather than zero, and
/// `None` once it has passed.
#[test]
fn minutes_until_rounds_up_and_reads_none_once_the_time_has_passed() {
    assert_eq!(minutes_until(420, 0), Some(7));
    assert_eq!(minutes_until(59, 0), Some(1));
    assert_eq!(minutes_until(60, 0), Some(1));
    assert_eq!(minutes_until(61, 0), Some(2));
    assert_eq!(minutes_until(100, 100), None);
    assert_eq!(minutes_until(99, 100), None);
}

#[test]
fn minute_word_pluralizes() {
    assert_eq!(minute_word(1), "1 minute");
    assert_eq!(minute_word(7), "7 minutes");
}

/// Every response is wrapped in one shell: the doctype, the title, and a
/// color-scheme meta fixed to dark, whatever the phone's own appearance is.
#[test]
fn the_shell_carries_the_doctype_title_and_color_scheme_meta() {
    let rendered = shell("<p>body</p>");
    assert!(rendered.starts_with("<!doctype html>"), "{rendered}");
    assert!(
        rendered.contains("<title>pns failures</title>"),
        "{rendered}"
    );
    assert!(
        rendered.contains("<meta name=\"color-scheme\" content=\"dark\">"),
        "{rendered}"
    );
    assert!(rendered.contains("<p>body</p>"), "{rendered}");
}

/// THE PAGE IS DARK ALWAYS. No `light-dark()` and no `prefers-color-scheme`
/// anywhere in the shell, so a phone in light mode still gets the dark card:
/// the operator's choice, not a fallback the browser could override.
#[test]
fn the_shell_is_dark_only() {
    let rendered = shell("<p>body</p>");
    assert!(rendered.contains(":root{color-scheme:dark}"), "{rendered}");
    assert!(rendered.contains("background:#0f1215"), "{rendered}");
    assert!(rendered.contains("color:#edf0f3"), "{rendered}");
    assert!(!rendered.contains("light-dark("), "{rendered}");
    assert!(!rendered.contains("prefers-color-scheme"), "{rendered}");
}
