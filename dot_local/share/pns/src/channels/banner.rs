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

use super::{Channel, Event};
use crate::probes::{FocusedPaneProbe, IdleProbe};
use crate::routing::Mode;
use crate::system::CommandRunner;

/// The bundle id the click activates when the pane's terminal is unknown.
pub const DEFAULT_TERMINAL_BUNDLE_ID: &str = "com.mitchellh.ghostty";

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
    let _ = (
        idle_secs,
        desk_idle_secs,
        terminal_id,
        front_bundle_id,
        focused_pane,
        pane,
    );
    todo!("R2e: all three must hold; any unknown fires the banner")
}

/// The shell string the click runs: focus the pane's WORKSPACE (the pane id
/// prefix before the first colon), then the pane, both through an absolute
/// herdr path because the click runs in a bare launchd context. No pane or
/// no herdr leaves the no-op `:` so `-activate` still raises the terminal.
pub fn click_command(herdr_path: Option<&str>, pane: &str) -> String {
    let _ = (herdr_path, pane);
    todo!("R2e: workspace focus then agent focus, or the no-op")
}

/// The frontmost app's bundle id, parsed out of `lsappinfo info -only
/// bundleid` output: the value of the quoted CFBundleIdentifier pair, from
/// the first line carrying one. Anything else is unknown.
pub fn parse_front_bundle_id(lsappinfo_output: &str) -> Option<String> {
    let _ = lsappinfo_output;
    todo!("R2e: the quoted CFBundleIdentifier value")
}

/// The exact terminal-notifier argv, order pinned: title, message, sound,
/// activate, execute.
pub fn notifier_args(title: &str, preview: &str, activate: &str, exec_cmd: &str) -> Vec<String> {
    let _ = (title, preview, activate, exec_cmd);
    todo!("R2e: the five flag pairs in the bash order")
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
}

impl<R: CommandRunner, P: IdleProbe + FocusedPaneProbe> Channel for BannerChannel<R, P> {
    fn deliver(&self, event: &Event, mode: Mode) {
        let _ = (event, mode);
        todo!("R2e: suppress only when watching, else spawn the notifier")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BannerChannel, DEFAULT_TERMINAL_BUNDLE_ID, click_command, notifier_args,
        operator_is_watching, parse_front_bundle_id,
    };
    use crate::channels::{Channel, Event};
    use crate::probes::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};
    use crate::routing::Mode;
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
                "a title",
                "-message",
                "a preview",
                "-sound",
                "default",
                "-activate",
                "com.term",
                "-execute",
                ": ",
            ]
        );
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
        };
        channel.deliver(&event_with_pane("wW:p1"), Mode::Async);
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
        };
        channel.deliver(&event_with_pane("wW:p1"), Mode::Async);
        let calls = channel.runner.calls.borrow();
        let notifier = calls
            .iter()
            .find(|call| call.contains("terminal-notifier"))
            .expect("an unwatched pane must fire the banner");
        assert!(notifier.contains("-title claude done: dotfiles"));
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
        };
        channel.deliver(&event_with_pane("wW:p1"), Mode::Async);
        let calls = channel.runner.calls.borrow();
        let notifier = calls
            .iter()
            .find(|call| call.contains("terminal-notifier"))
            .expect("unknown terminal fires");
        assert!(notifier.contains(&format!("-activate {DEFAULT_TERMINAL_BUNDLE_ID}")));
    }

    // --- dispatch precedence --------------------------------------------------

    #[test]
    fn an_explicit_channels_dir_means_executables_win() {
        assert!(!crate::channels::native_first(true));
        assert!(crate::channels::native_first(false));
    }
}
