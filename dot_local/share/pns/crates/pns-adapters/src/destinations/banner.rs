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
use pns_application::CommandRunner;
use pns_domain::routing::ReportMode;

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
    /// WHETHER THE SPAWN ANSWERED, which is the whole of what this channel can
    /// know: a banner has no second surface to report itself on, and the
    /// runner answers nothing for a notifier that is not installed and for one
    /// killed at its deadline alike.
    ///
    /// NO EVENT HEARS IT. `ReportOutcome` is produced only under
    /// `--remote-only`, which selects durable plugins, and this one is not
    /// durable, so the sentence is unreachable from an event's stdout.
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
        // guard, and a runner that cannot find it costs the caller nothing.
        match self.runner.run(
            "terminal-notifier",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        ) {
            Some(_) => Delivery::Delivered("posted the banner".to_string()),
            // NAMED, because the remedy is installing that one binary and a
            // line that only said "failed" would send the operator looking at
            // the notification settings instead.
            None => Delivery::Failed("banner FAILED (terminal-notifier did not run)".to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
