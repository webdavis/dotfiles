//! The clickable macOS banner, native. Clicking focuses the exact herdr pane
//! the event came from, which is the whole reason this channel beats a plain
//! notification. Delivery spawns `terminal-notifier`, the one binary that can
//! post a banner without a signed app bundle; everything around that spawn is
//! pure and pinned.
//!
//! SUPPRESSION FAILS OPEN. The banner is suppressed only when all three hold
//! at once: the Mac was touched recently, the pane's terminal is the
//! frontmost app, and the pane is herdr's focused pane. Anything false,
//! unreadable or unknown FIRES the banner: a spare banner is spam, a dropped
//! one is a lost notification.

use super::{Delivery, Event};
use crate::probes::{FocusedPaneProbe, IdleProbe};
use crate::routing::ReportMode;
use crate::system::CommandRunner;

/// The bundle id the click activates when the pane's terminal is unknown.
pub const DEFAULT_TERMINAL_BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// Absolute, like the other system readers: a channel must not resolve a
/// system binary through a PATH it does not control.
const LSAPPINFO_PATH: &str = "/usr/bin/lsappinfo";

/// True when the operator is demonstrably watching this pane RIGHT NOW, the
/// one case the banner may be dropped. Pure: every reading arrives as an
/// argument, every unknown is an `Option::None` or empty string, and any
/// unknown answer is false, which fires.
pub fn operator_is_watching(
    idle_secs: Option<u64>,
    desk_idle_secs: Option<u64>,
    terminal_id: &str,
    front_bundle_id: Option<&str>,
    focused_pane: Option<&str>,
    pane: &str,
) -> bool {
    if pane.is_empty() {
        return false;
    }
    let (Some(idle_secs), Some(desk_idle_secs)) = (idle_secs, desk_idle_secs) else {
        return false;
    };
    idle_secs < desk_idle_secs
        && !terminal_id.is_empty()
        && front_bundle_id == Some(terminal_id)
        && focused_pane == Some(pane)
}

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

/// The frontmost app's bundle id, parsed out of `lsappinfo info -only
/// bundleid` output: the value of the quoted CFBundleIdentifier pair, from
/// the first line carrying one. Anything else is unknown.
pub fn parse_front_bundle_id(lsappinfo_output: &str) -> Option<String> {
    // The QUOTED key exactly, as the bash sed requires: an unquoted lookalike
    // must not produce a confident id that wrongly suppresses.
    let value = lsappinfo_output
        .split_once("\"CFBundleIdentifier\"=")?
        .1
        .strip_prefix('"')?;
    Some(value[..value.find('"')?].to_string())
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

/// The native banner plugin. The runner spawns `lsappinfo` and
/// `terminal-notifier`; the probes are the same seams the engine reads.
pub struct BannerChannel<R: CommandRunner, P> {
    pub runner: R,
    pub probes: P,
    /// `PNS_TERMINAL_BUNDLE_ID` override, else the inherited
    /// `__CFBundleIdentifier`, else empty (unknown never suppresses).
    pub terminal_id: String,
    pub desk_idle_secs: Option<u64>,
    /// Absolute herdr path resolved at composition time, `None` when PATH
    /// has none.
    pub herdr_path: Option<String>,
    /// `RELAY_IDLE_SECS`: a caller-supplied idle beats the probe, and the
    /// probe is not read when it is present.
    pub idle_override: Option<u64>,
    /// `RELAY_HERDR_FOCUSED_PANE`: same, for the focused pane.
    pub focused_override: Option<String>,
}

impl<R: CommandRunner, P: IdleProbe + FocusedPaneProbe> BannerChannel<R, P> {
    /// Always silent: a banner that did not post has no second surface to
    /// report itself on.
    pub fn deliver(&self, event: &Event, _mode: ReportMode) -> Delivery {
        // Two steps, as the bash runs them: the frontmost app's ASN, then its
        // bundle id. Either step unreadable leaves the id unknown, which fires.
        let front_bundle_id = self
            .runner
            .run(LSAPPINFO_PATH, &["front"])
            .map(|asn| asn.trim().to_string())
            .filter(|asn| !asn.is_empty())
            .and_then(|asn| {
                self.runner
                    .run(LSAPPINFO_PATH, &["info", "-only", "bundleid", &asn])
            })
            .and_then(|output| parse_front_bundle_id(&output));
        // A caller-supplied reading IS the reading, so the probe under it is
        // never consulted.
        let idle_secs = match self.idle_override {
            Some(secs) => Some(secs),
            None => self.probes.idle_secs(),
        };
        let focused_pane = match &self.focused_override {
            Some(pane) => Some(pane.clone()),
            None => self.probes.focused_pane(),
        };

        if operator_is_watching(
            idle_secs,
            self.desk_idle_secs,
            &self.terminal_id,
            front_bundle_id.as_deref(),
            focused_pane.as_deref(),
            &event.pane,
        ) {
            return Delivery::Silent;
        }

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
        BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args,
        operator_is_watching, parse_front_bundle_id, verbatim_argument,
    };
    use crate::channels::Event;
    use crate::probes::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};
    use crate::routing::ReportMode;
    use crate::system::CommandRunner;
    use std::cell::RefCell;

    // --- suppression: all three, fail open ----------------------------------

    const WATCHING: (&str, &str) = ("com.term", "com.term");

    fn watching_case(
        idle: Option<u64>,
        terminal: &str,
        front: Option<&str>,
        focused: Option<&str>,
        pane: &str,
    ) -> bool {
        operator_is_watching(idle, Some(120), terminal, front, focused, pane)
    }

    #[test]
    fn all_three_holding_at_once_suppresses_the_banner() {
        assert!(watching_case(
            Some(5),
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
    }

    #[test]
    fn a_stale_keyboard_fires_even_with_the_pane_front_and_focused() {
        assert!(!watching_case(
            Some(900),
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
    }

    #[test]
    fn a_buried_terminal_fires_even_with_the_pane_focused() {
        assert!(!watching_case(
            Some(5),
            "com.term",
            Some("com.browser"),
            Some("wW:p1"),
            "wW:p1"
        ));
    }

    #[test]
    fn an_unfocused_pane_fires_even_with_the_terminal_front() {
        assert!(!watching_case(
            Some(5),
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p2"),
            "wW:p1"
        ));
    }

    #[test]
    fn every_unknown_reading_fires_because_a_dropped_banner_costs_the_event() {
        // Unknown idle, unknown terminal, unknown front app, unknown focused
        // pane, and a pane-less event each refuse to suppress.
        assert!(!watching_case(
            None,
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
        assert!(!watching_case(
            Some(5),
            "",
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
        assert!(!watching_case(
            Some(5),
            WATCHING.0,
            None,
            Some("wW:p1"),
            "wW:p1"
        ));
        assert!(!watching_case(
            Some(5),
            WATCHING.0,
            Some(WATCHING.1),
            None,
            "wW:p1"
        ));
        assert!(!watching_case(
            Some(5),
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            ""
        ));
    }

    #[test]
    fn an_unreadable_desk_threshold_fires_too() {
        assert!(!operator_is_watching(
            Some(5),
            None,
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
    }

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

    // --- the lsappinfo parser -------------------------------------------------

    #[test]
    fn the_front_bundle_id_is_the_quoted_identifier_value() {
        let output = "\"CFBundleIdentifier\"=\"com.mitchellh.ghostty\"\n";
        assert_eq!(
            parse_front_bundle_id(output),
            Some("com.mitchellh.ghostty".to_string())
        );
    }

    #[test]
    fn unrecognisable_lsappinfo_output_reads_as_unknown_which_fires() {
        assert_eq!(parse_front_bundle_id(""), None);
        assert_eq!(parse_front_bundle_id("no identifier here"), None);
    }

    // --- the notifier argv ----------------------------------------------------

    #[test]
    fn the_notifier_argv_carries_the_five_flag_pairs_in_the_bash_order() {
        assert_eq!(
            notifier_args("a title", "a preview", "com.term", ": "),
            vec![
                "-title",
                "\\a title",
                "-message",
                "\\a preview",
                "-sound",
                "default",
                "-activate",
                "com.term",
                "-execute",
                ": ",
            ]
        );
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

    // --- the plugin end to end, through fakes ---------------------------------

    struct ScriptedRunner {
        lsappinfo: Option<String>,
        calls: RefCell<Vec<String>>,
    }

    impl CommandRunner for ScriptedRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
            if program.contains("lsappinfo") {
                self.lsappinfo.clone()
            } else {
                Some(String::new())
            }
        }
    }

    struct FixedProbes {
        idle: Option<u64>,
        focused: Option<String>,
    }
    impl IdleProbe for FixedProbes {
        fn idle_secs(&self) -> Option<u64> {
            self.idle
        }
    }
    impl PhoneMarkerProbe for FixedProbes {
        fn marker_mtime_secs(&self) -> Option<u64> {
            None
        }
    }
    impl MoshRateProbe for FixedProbes {
        fn sample_csv(&self) -> Option<String> {
            None
        }
    }
    impl FocusedPaneProbe for FixedProbes {
        fn focused_pane(&self) -> Option<String> {
            self.focused.clone()
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
    fn a_watched_pane_spawns_no_notifier_at_all() {
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("\"CFBundleIdentifier\"=\"com.term\"\n".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: "com.term".to_string(),
            desk_idle_secs: Some(120),
            herdr_path: Some("/x/herdr".to_string()),
            idle_override: None,
            focused_override: None,
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        assert!(
            !calls.iter().any(|call| call.contains("terminal-notifier")),
            "suppressed delivery must not spawn the notifier: {calls:?}"
        );
    }

    #[test]
    fn an_unwatched_pane_fires_the_notifier_with_the_click_baked_in() {
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("\"CFBundleIdentifier\"=\"com.browser\"\n".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: "com.term".to_string(),
            desk_idle_secs: Some(120),
            herdr_path: Some("/x/herdr".to_string()),
            idle_override: None,
            focused_override: None,
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        let notifier = calls
            .iter()
            .find(|call| call.contains("terminal-notifier"))
            .expect("an unwatched pane must fire the banner");
        // Armored, as it reaches the real spawn: the backslash is stripped by
        // terminal-notifier's own plist-literal parsing.
        assert!(notifier.contains("-title \\claude done: dotfiles"));
        assert!(notifier.contains("-activate com.term"));
        assert!(notifier.contains("/x/herdr workspace focus wW; /x/herdr agent focus wW:p1"));
    }

    #[test]
    fn an_unknown_terminal_activates_the_default_and_never_suppresses() {
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("\"CFBundleIdentifier\"=\"com.term\"\n".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: String::new(),
            desk_idle_secs: Some(120),
            herdr_path: None,
            idle_override: None,
            focused_override: None,
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        let notifier = calls
            .iter()
            .find(|call| call.contains("terminal-notifier"))
            .expect("unknown terminal fires");
        assert!(notifier.contains(&format!("-activate {DEFAULT_TERMINAL_BUNDLE_ID}")));
    }

    #[test]
    fn idle_exactly_at_the_threshold_is_already_away_so_the_banner_fires() {
        // The same boundary the engine uses: at the threshold the operator
        // counts as away, and the two verdicts must not disagree by one
        // second.
        assert!(!watching_case(
            Some(120),
            WATCHING.0,
            Some(WATCHING.1),
            Some("wW:p1"),
            "wW:p1"
        ));
    }

    #[test]
    fn an_unquoted_identifier_key_reads_as_unknown_like_the_bash_sed() {
        // The bash pattern requires the QUOTED key; a lookalike line must not
        // produce a confident id that wrongly suppresses.
        assert_eq!(
            super::parse_front_bundle_id("CFBundleIdentifier=\"com.term\"\n"),
            None
        );
    }

    #[test]
    fn a_caller_supplied_idle_beats_the_probe_and_spares_the_read() {
        // RELAY_IDLE_SECS=900 with a live idle of 5: the bash fires because
        // the override IS the reading; consulting the probe instead would
        // suppress a banner the caller asked to judge as away.
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("\"CFBundleIdentifier\"=\"com.term\"\n".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: "com.term".to_string(),
            desk_idle_secs: Some(120),
            herdr_path: None,
            idle_override: Some(900),
            focused_override: None,
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        assert!(
            calls.iter().any(|call| call.contains("terminal-notifier")),
            "the override says away, so the banner fires: {calls:?}"
        );
    }

    #[test]
    fn a_caller_supplied_focused_pane_beats_the_probe() {
        // RELAY_HERDR_FOCUSED_PANE=wW:p2 with live focus on the event's own
        // pane: the caller's reading wins, the panes differ, the banner fires.
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("\"CFBundleIdentifier\"=\"com.term\"\n".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: "com.term".to_string(),
            desk_idle_secs: Some(120),
            herdr_path: None,
            idle_override: None,
            focused_override: Some("wW:p2".to_string()),
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        assert!(
            calls.iter().any(|call| call.contains("terminal-notifier")),
            "the caller's focus reading wins: {calls:?}"
        );
    }

    #[test]
    fn the_front_app_is_read_in_the_bash_two_step_and_nothing_else() {
        // `lsappinfo front` for the ASN, then `info -only bundleid` ON that
        // ASN: a collapsed call reads an ARBITRARY app and can wrongly
        // suppress with the terminal buried.
        let channel = BannerChannel {
            runner: ScriptedRunner {
                lsappinfo: Some("ASN-42".to_string()),
                calls: RefCell::new(Vec::new()),
            },
            probes: FixedProbes {
                idle: Some(5),
                focused: Some("wW:p1".to_string()),
            },
            terminal_id: "com.term".to_string(),
            desk_idle_secs: Some(120),
            herdr_path: None,
            idle_override: None,
            focused_override: None,
        };
        channel.deliver(&event_with_pane("wW:p1"), ReportMode::Silent);
        let calls = channel.runner.calls.borrow();
        assert_eq!(calls[0], "/usr/bin/lsappinfo front");
        assert_eq!(calls[1], "/usr/bin/lsappinfo info -only bundleid ASN-42");
    }

    // --- dispatch precedence --------------------------------------------------

    #[test]
    fn an_explicit_channels_dir_means_executables_win() {
        assert!(!crate::channels::native_first(true));
        assert!(crate::channels::native_first(false));
    }
}
