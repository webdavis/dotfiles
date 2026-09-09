use super::*;

const HERDR: &str = "/Users/o/.local/bin/herdr";
const PNS: &str = "/Users/o/.cargo/bin/pns";

/// The string the banner runs is FIXED. It is rendered by a shell, so anything
/// composed into it from an event would be code; only a number pns assigned
/// crosses that line.
#[test]
fn the_click_command_is_a_literal_and_the_id() {
    assert_eq!(click_command(47), "pns click 47");
}

/// An operator running herdr wants the pane; one who is not has no session for
/// a pane to open in. A configured default would be wrong for half of them.
#[test]
fn the_view_is_inferred_from_whether_herdr_is_there() {
    assert_eq!(ClickView::inferred(true), ClickView::Herdr);
    assert_eq!(ClickView::inferred(false), ClickView::Window);
}

/// The herdr view opens the record beside what the operator is already looking
/// at, which is the smallest interruption available.
#[test]
fn the_herdr_view_splits_a_pane_running_the_detail_view() {
    assert_eq!(
        ClickView::Herdr.argv(47, HERDR, PNS).unwrap(),
        [
            HERDR,
            "pane",
            "split",
            "--command",
            "/Users/o/.cargo/bin/pns failures 47"
        ]
    );
}

/// NEVER `ghostty -e`. Ghostty's own help says `-e` is unsupported on macOS,
/// where the binary hands off to the running app rather than starting a
/// session, and `-na` is what forces a new window rather than raising one the
/// operator was already reading.
#[test]
fn the_window_view_opens_a_new_ghostty_through_open_rather_than_the_binary() {
    let argv = ClickView::Window.argv(47, HERDR, PNS).unwrap();
    assert_eq!(argv[0], "/usr/bin/open");
    assert!(argv.contains(&"-na".to_string()), "{argv:?}");
    assert!(argv.contains(&"Ghostty.app".to_string()), "{argv:?}");
    assert_eq!(argv.last().unwrap(), "/Users/o/.cargo/bin/pns failures 47");
    assert!(
        !argv[0].contains("ghostty"),
        "the binary is not the entry point"
    );
}

/// The operator's own command, with the id substituted wherever they put the
/// placeholder, including more than once.
#[test]
fn a_configured_command_substitutes_the_id_at_every_placeholder() {
    let view = ClickView::Command("/usr/bin/log show --predicate {id} --last {id}".into());
    assert_eq!(
        view.argv(47, HERDR, PNS).unwrap(),
        ["/usr/bin/log", "show", "--predicate", "47", "--last", "47"]
    );
}

/// A command view whose string was never written is not a view. Running an
/// empty argv would report success for a window that never opened.
#[test]
fn a_command_view_with_nothing_in_it_produces_no_argv() {
    assert_eq!(ClickView::Command(String::new()).argv(47, HERDR, PNS), None);
    assert_eq!(ClickView::Command("   ".into()).argv(47, HERDR, PNS), None);
}

/// A typo is REFUSED rather than falling back, because a click that quietly
/// opened something else is one the operator believes is configured.
#[test]
fn an_unknown_click_type_is_refused_and_the_message_names_the_three() {
    let error = parse_view("herd", "", true).unwrap_err();
    assert!(error.contains("\"herd\""), "{error}");
    for named in ["herdr", "window", "command"] {
        assert!(error.contains(named), "{error} omits {named}");
    }
}

/// `command` with no command is the same mistake in a different shape, and is
/// refused with the key that is missing named.
#[test]
fn a_command_type_with_no_command_is_refused_by_name() {
    let error = parse_view("command", "  ", true).unwrap_err();
    assert!(error.contains("click_command"), "{error}");
}

/// An unwritten key falls to the inference, which is the whole point of having
/// one: a fresh machine needs no config for a click to work.
#[test]
fn an_unwritten_type_falls_to_the_inference_in_both_directions() {
    assert_eq!(parse_view("", "", true).unwrap(), ClickView::Herdr);
    assert_eq!(parse_view("  ", "", false).unwrap(), ClickView::Window);
}

/// The three names parse to the three views, so the config's vocabulary and the
/// code's agree.
#[test]
fn each_named_type_parses_to_its_view() {
    assert_eq!(parse_view("herdr", "", false).unwrap(), ClickView::Herdr);
    assert_eq!(parse_view("window", "", true).unwrap(), ClickView::Window);
    assert_eq!(
        parse_view("command", "/bin/echo {id}", true).unwrap(),
        ClickView::Command("/bin/echo {id}".into())
    );
}
