use super::*;

fn parsed(argv: &[&str]) -> Option<Options> {
    options(
        &argv
            .iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn a_bare_recap_names_no_window_at_all() {
    assert_eq!(
        parsed(&[]).expect("a bare recap").span,
        Span::MostRecentlyEnded
    );
}

#[test]
fn a_window_word_names_its_window_and_previous_steps_it_back() {
    assert_eq!(
        parsed(&["morning"]).expect("a window").span,
        Span::Named {
            window: "morning".to_string(),
            previous: false
        }
    );
    assert_eq!(
        parsed(&["week", "--previous"]).expect("a window").span,
        Span::Named {
            window: "week".to_string(),
            previous: true
        }
    );
}

#[test]
fn previous_with_no_window_is_refused() {
    // NOTHING TO STEP BACK FROM. Ignoring it would answer a different
    // question from the one that was asked.
    assert_eq!(parsed(&["--previous"]), None);
    assert_eq!(parsed(&["--since", "2h", "--previous"]), None);
    assert_eq!(parsed(&["open", "--previous"]), None);
}

#[test]
fn two_span_flags_together_are_refused_in_either_order() {
    for argv in [
        ["morning", "--since", "2h"].as_slice(),
        ["--since", "2h", "morning"].as_slice(),
        ["morning", "--duration", "2h"].as_slice(),
        ["--duration", "2h", "--since", "2h"].as_slice(),
        ["open", "--since", "2h"].as_slice(),
        ["today", "week"].as_slice(),
    ] {
        assert_eq!(parsed(argv), None, "{argv:?}");
    }
}

#[test]
fn open_takes_no_window_and_is_its_own_span() {
    assert_eq!(parsed(&["open"]).expect("open").span, Span::Open);
    assert_eq!(
        parsed(&["open", "--limit", "3"]).expect("open").limit,
        Some(3)
    );
}

#[test]
fn a_word_that_is_no_window_is_refused_rather_than_read_as_something_else() {
    for argv in [
        ["yesterdya"].as_slice(),
        ["--recent", "5"].as_slice(),
        ["-x"].as_slice(),
    ] {
        assert_eq!(parsed(argv), None, "{argv:?}");
    }
}

#[test]
fn the_document_flags_select_the_form_and_the_mask_selects_it_too() {
    assert_eq!(parsed(&["--json"]).expect("json").format, Some(Wire::Json));
    assert_eq!(parsed(&["--toon"]).expect("toon").format, Some(Wire::Toon));
    let masked = parsed(&["--schema", "mask.toon"]).expect("a mask");
    assert_eq!(masked.schema.as_deref(), Some("mask.toon"));
    assert_eq!(masked.format, None, "the mask's own format decides");
    let overridden = parsed(&["--json", "--schema", "mask.toon"]).expect("a mask");
    assert_eq!(overridden.format, Some(Wire::Json));
}

#[test]
fn sections_repeat_and_a_limit_of_nothing_is_refused() {
    let named = parsed(&["--section", "open", "--section", "tasks"]).expect("sections");
    assert_eq!(named.sections, ["open", "tasks"]);
    assert_eq!(parsed(&["--limit", "0"]), None);
    assert_eq!(parsed(&["--limit", "x"]), None);
}

#[test]
fn a_flag_with_no_value_is_refused_rather_than_read_as_a_window() {
    for argv in [
        ["--section"].as_slice(),
        ["--to"].as_slice(),
        ["--schema"].as_slice(),
        ["--since"].as_slice(),
    ] {
        assert_eq!(parsed(argv), None, "{argv:?}");
    }
}
