use super::{
    BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args, verbatim_argument,
};
use crate::destinations::Event;
use pns_application::CommandRunner;
use pns_domain::routing::ReportMode;
use std::cell::RefCell;

// --- the click string ----------------------------------------------------

#[test]
fn the_click_focuses_the_workspace_then_the_pane_through_the_absolute_path() {
    assert_eq!(
        click_command(Some("/Users/o/.local/bin/herdr"), "wW:p21"),
        "/Users/o/.local/bin/herdr workspace focus wW; /Users/o/.local/bin/herdr agent focus wW:p21"
    );
}

#[test]
fn no_pane_or_no_herdr_leaves_the_noop_so_activate_still_raises_the_terminal() {
    assert_eq!(click_command(Some("/bin/herdr"), ""), ":");
    assert_eq!(click_command(None, "wW:p21"), ":");
}

// --- the encoding itself --------------------------------------------------

/// The cases, defined ONCE and driven by both tests below: a label, the
/// intended text, and the exact argv value it must encode to. The
/// expectations are written out byte for byte rather than derived from the
/// implementation, so they cannot drift with it, and every assertion
/// carries the label so a failure names its case rather than an index.
///
/// The inputs are the shapes the live probes fired: the six characters the
/// parser eats, the two near misses that fooled the earlier reading (a
/// leading space, a zero-width space), the two controls that never needed
/// encoding (a digit, a letter), a value that already starts with a
/// backslash, and the empty string.
const ENCODING_MATRIX: [(&str, &str, &str); 12] = [
    ("leading paren", "(leading paren", "\\(leading paren"),
    ("leading bracket", "[leading bracket", "\\[leading bracket"),
    ("leading brace", "{leading brace", "\\{leading brace"),
    ("leading dash", "-leading dash", "\\-leading dash"),
    ("leading angle", "<leading angle", "\\<leading angle"),
    ("leading quote", "\"leading quote", "\\\"leading quote"),
    ("leading space", " leading space", "\\ leading space"),
    ("leading digit", "9 leading digit", "\\9 leading digit"),
    ("leading letter", "a leading letter", "\\a leading letter"),
    (
        "leading backslash",
        "\\a leading backslash",
        "\\\\a leading backslash",
    ),
    (
        "leading zero-width space",
        "\u{200b}zero width space",
        "\\\u{200b}zero width space",
    ),
    ("empty string", "", "\\"),
];

/// The first characters measured to yield no string at all from the
/// argument-domain parsing (probes P4-P8, 2026-08-12).
const EATEN_FIRST_CHARACTERS: [char; 7] = ['(', '[', '{', '-', '<', '"', '\u{200b}'];

#[test]
fn every_case_in_the_matrix_encodes_to_its_exact_argv_value() {
    for (case, intended, expected) in ENCODING_MATRIX {
        assert_eq!(
            verbatim_argument(intended),
            expected,
            "case {case}: intended text {intended:?} encoded wrong"
        );
    }
}

#[test]
fn no_case_in_the_matrix_can_encode_to_a_value_the_parser_eats() {
    // The inversion of what the probe measured: instead of asserting what
    // Apple's parser does with each shape, assert that our encoding never
    // hands it one of the shapes it cannot read. The first character is
    // always the backslash, whatever the text was, so the eaten set below
    // is unreachable by construction and stays unreachable if that set
    // ever grows.
    for (case, intended, _) in ENCODING_MATRIX {
        let encoded = verbatim_argument(intended);
        let first = encoded.chars().next().unwrap_or_else(|| {
            panic!(
                "case {case}: an encoded value is never empty, the prefix alone is one character"
            )
        });
        assert_eq!(
            first, '\\',
            "case {case}: intended text {intended:?} encoded to {encoded:?}, which does not lead with the escape"
        );
        assert!(
            !EATEN_FIRST_CHARACTERS.contains(&first),
            "case {case}: intended text {intended:?} encoded to {encoded:?}, which leads with a character the parser eats"
        );
    }
}

#[test]
fn a_branchless_message_starting_with_a_killer_character_is_still_encoded() {
    // The case render.rs used to assert it prevented, in the one place
    // that actually prevents it: with no branch to prefix it, the detail
    // IS the message and can lead with anything the operator typed.
    let composed = pns_domain::render::message("", "(a parenthesised detail", "done");
    assert_eq!(composed, "(a parenthesised detail");
    assert_eq!(
        notifier_args("t", &composed, "com.term", ": ")[3],
        "\\(a parenthesised detail"
    );
}

#[test]
fn the_message_is_encoded_on_the_same_terms_as_the_title() {
    // Both are operator-facing text read through the identical parsing, so
    // a message beginning with a killer character needs the encoding just
    // as much as a title does.
    let args = notifier_args("(a title", "[a preview", "com.term", ": ");
    assert_eq!(args[1], "\\(a title");
    assert_eq!(args[3], "\\[a preview");
}

// --- the plugin end to end, through a fake runner -----------------------

struct RecordingRunner {
    /// Whether the spawn answered. `None` is the whole of what the runner
    /// reports about a notifier that is not installed or that outlived its
    /// deadline, and it is scripted here for the same reason hermes's post
    /// is: a failure no double can produce is a failure no test can see.
    answers: bool,
    calls: RefCell<Vec<String>>,
}

impl RecordingRunner {
    fn answering(answers: bool) -> Self {
        RecordingRunner {
            answers,
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl CommandRunner for RecordingRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", args.join(" ")));
        self.answers.then(String::new)
    }
}

fn channel(terminal_id: &str, herdr_path: Option<&str>) -> BannerChannel<RecordingRunner> {
    BannerChannel {
        runner: RecordingRunner::answering(true),
        terminal_id: terminal_id.to_string(),
        herdr_path: herdr_path.map(String::from),
    }
}

fn event_with_pane(pane: &str) -> Event {
    Event {
        title: "claude done: dotfiles".to_string(),
        preview: "a preview".to_string(),
        pane: pane.to_string(),
        ..Event::default()
    }
}

#[test]
fn a_delivered_leg_posts_the_banner_with_the_click_baked_in() {
    // The channel no longer decides anything: handed a leg, it fires.
    // Whether it deserved one is the plan's call, made before this.
    let banner = channel("com.term", Some("/x/herdr"));
    banner.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
    let calls = banner.runner.calls.borrow();
    let notifier = calls
        .iter()
        .find(|call| call.contains("terminal-notifier"))
        .expect("a delivered leg fires");
    assert!(notifier.contains("-title \\claude done: dotfiles"));
    assert!(notifier.contains("-activate com.term"));
    assert!(notifier.contains("/x/herdr workspace focus wW; /x/herdr agent focus wW:p1"));
}

#[test]
fn nothing_but_the_notifier_is_ever_spawned() {
    // It used to read the frontmost app to judge suppression for itself,
    // which meant two places could disagree about one event.
    let banner = channel("com.term", None);
    banner.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
    let calls = banner.runner.calls.borrow();
    assert_eq!(calls.len(), 1, "one spawn only: {calls:?}");
    assert!(calls[0].starts_with("terminal-notifier"));
}

#[test]
fn a_spawn_that_answered_is_delivered_and_one_that_never_ran_names_the_notifier() {
    // The banner has no second surface to report itself on, so the ONLY
    // evidence that it posted is that the spawn answered at all. A runner
    // answering nothing is terminal-notifier missing from PATH or killed
    // at its deadline, and the sentence names the binary to install.
    for (answered, verdict) in [
        (
            true,
            crate::destinations::Delivery::Delivered("posted the banner".to_string()),
        ),
        (
            false,
            crate::destinations::Delivery::Failed(
                "banner FAILED (terminal-notifier did not run)".to_string(),
            ),
        ),
    ] {
        let banner = BannerChannel {
            runner: RecordingRunner::answering(answered),
            terminal_id: "com.term".to_string(),
            herdr_path: None,
        };
        assert_eq!(
            banner.deliver(&event_with_pane("wW:p1"), ReportMode::Silent),
            verdict,
            "answered: {answered}"
        );
    }
}

#[test]
fn an_unknown_terminal_activates_the_default() {
    let banner = channel("", None);
    banner.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
    let calls = banner.runner.calls.borrow();
    assert!(calls[0].contains(&format!("-activate {DEFAULT_TERMINAL_BUNDLE_ID}")));
}

// --- dispatch precedence --------------------------------------------------

#[test]
fn an_explicit_channels_dir_means_executables_win() {
    assert!(!crate::destinations::native_first(true));
    assert!(crate::destinations::native_first(false));
}
