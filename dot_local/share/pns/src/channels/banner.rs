//! The clickable macOS banner, native. Clicking focuses the exact herdr pane
//! the event came from, which is the whole reason this channel beats a plain
//! notification. Delivery spawns `terminal-notifier`, the one binary that can
//! post a banner without a signed app bundle; everything around that spawn is
//! pure and pinned.
//!
//! DELIVERY ONLY. Whether a banner is warranted is the PLAN's decision, made
//! once per event from the surface and visibility model; this channel fires
//! when it is handed a leg and never second-guesses it. It used to run its
//! own three-part suppression here, which meant two places could disagree
//! about the same event.

use super::{Delivery, Event};
use crate::routing::ReportMode;
use crate::system::CommandRunner;

/// The bundle id the click activates when the pane's terminal is unknown.
pub const DEFAULT_TERMINAL_BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// The shell string the click runs: focus the pane's WORKSPACE (the pane id
/// prefix before the first colon), then the pane, both through an absolute
/// herdr path because the click runs in a bare launchd context. No pane or
/// no herdr leaves the no-op `:` so `-activate` still raises the terminal.
pub fn click_command(herdr_path: Option<&str>, pane: &str) -> String {
    match herdr_path {
        Some(herdr) if !pane.is_empty() => {
            let workspace = pane.split(':').next().unwrap_or(pane);
            format!("{herdr} workspace focus {workspace}; {herdr} agent focus {pane}")
        }
        _ => ":".to_string(),
    }
}

/// Intended text in, an argv value terminal-notifier renders VERBATIM out.
/// This one function is the whole of what we know about how that dependency
/// ingests an option value, so a change in the rule is a change to this file
/// and nothing else.
///
/// The contract, in two halves, both measured live on 2026-08-12 (probes
/// P4-P8, matrix in the session ledger; the drill re-measures it):
///
/// 1. terminal-notifier reads its options off NSUserDefaults' ARGUMENT DOMAIN,
///    so the value goes through the old-style property list parser before any
///    of its own code runs. A value whose FIRST character is "(", "[", "{",
///    "-", "<", a double quote or a zero-width space yields no string at all
///    there, and the banner then renders title-only.
/// 2. What survives has ONE leading backslash stripped. Upstream documents the
///    escape in `terminal-notifier -help` (2.0.0): "the first character of a
///    message has to be escaped in order to be recognized ... like so: '\\['".
///
/// So one unconditional leading backslash is exactly the encoding: it puts a
/// character the parser accepts in position one, and half 2 takes it back off.
/// Unconditional rather than applied to a character SET, because a set is a
/// list to keep in step with whatever else that parser eats, and the prefix
/// costs a value that already begins with a backslash nothing: it arrives
/// carrying exactly one.
pub fn verbatim_argument(text: &str) -> String {
    format!("\\{text}")
}

/// The exact terminal-notifier argv, order pinned: title, message, sound,
/// activate, execute. The title and the message are both operator-facing text,
/// so both go out through [`verbatim_argument`].
pub fn notifier_args(title: &str, preview: &str, activate: &str, exec_cmd: &str) -> Vec<String> {
    let encoded_title = verbatim_argument(title);
    let encoded_preview = verbatim_argument(preview);
    [
        "-title",
        encoded_title.as_str(),
        "-message",
        encoded_preview.as_str(),
        "-sound",
        "default",
        "-activate",
        activate,
        "-execute",
        exec_cmd,
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// The native banner plugin: a spawn, and the click that focuses the pane.
pub struct BannerChannel<R: CommandRunner> {
    pub runner: R,
    /// `PNS_TERMINAL_BUNDLE_ID` override, else the inherited
    /// `__CFBundleIdentifier`, else empty (unknown activates the default).
    pub terminal_id: String,
    /// Absolute herdr path resolved at composition time, `None` when PATH
    /// has none.
    pub herdr_path: Option<String>,
}

impl<R: CommandRunner> BannerChannel<R> {
    /// Always silent: a banner that did not post has no second surface to
    /// report itself on.
    pub fn deliver(&self, event: &Event, _mode: ReportMode) -> Delivery {
        let activate = if self.terminal_id.is_empty() {
            DEFAULT_TERMINAL_BUNDLE_ID
        } else {
            &self.terminal_id
        };
        let args = notifier_args(
            &event.title,
            &event.preview,
            activate,
            &click_command(self.herdr_path.as_deref(), &event.pane),
        );
        // By NAME, not an absolute path: parity with the bash's `command -v`
        // guard, and a runner that cannot find it is silently fine.
        self.runner.run(
            "terminal-notifier",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        Delivery::Silent
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args, verbatim_argument,
    };
    use crate::channels::Event;
    use crate::routing::ReportMode;
    use crate::system::CommandRunner;
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
                panic!("case {case}: an encoded value is never empty, the prefix alone is one character")
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
        let composed = crate::render::message("", "(a parenthesised detail", "done");
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
        calls: RefCell<Vec<String>>,
    }

    impl CommandRunner for RecordingRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
            Some(String::new())
        }
    }

    fn channel(terminal_id: &str, herdr_path: Option<&str>) -> BannerChannel<RecordingRunner> {
        BannerChannel {
            runner: RecordingRunner {
                calls: RefCell::new(Vec::new()),
            },
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
    fn an_unknown_terminal_activates_the_default() {
        let banner = channel("", None);
        banner.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = banner.runner.calls.borrow();
        assert!(calls[0].contains(&format!("-activate {DEFAULT_TERMINAL_BUNDLE_ID}")));
    }

    // --- dispatch precedence --------------------------------------------------

    #[test]
    fn an_explicit_channels_dir_means_executables_win() {
        assert!(!crate::channels::native_first(true));
        assert!(crate::channels::native_first(false));
    }
}
