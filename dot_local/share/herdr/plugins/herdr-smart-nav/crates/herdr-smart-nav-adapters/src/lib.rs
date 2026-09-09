//! herdr, as this plugin talks to it: one probe and one command, both through
//! the `herdr` binary rather than its socket.
//!
//! THE CLI AND NOT THE SOCKET, because a navigation press is one round trip on
//! a keystroke the operator is waiting for, and the binary is the interface
//! herdr documents. The sibling workspace-jump plugin speaks the socket because
//! it makes two requests per jump and can fall back here; this makes one.

use std::process::Command;

use herdr_smart_nav_domain::Action;
use herdr_smart_nav_protocol::is_nvim_foreground;

/// The `herdr` binary, at the path the composition root resolved.
pub struct Herdr {
    binary: String,
}

impl Herdr {
    pub fn new(binary: String) -> Self {
        Self { binary }
    }

    /// Whether the named pane has Neovim in the foreground.
    ///
    /// STDOUT IS TRUSTED EVEN ON A NON-ZERO EXIT, which is the legacy probe's
    /// own behavior kept deliberately: herdr has exited non-zero while still
    /// printing a well-formed answer, and the parse below refuses anything that
    /// is not one. A false here costs a pane move where a split move was
    /// wanted, never a wrong pane.
    pub fn is_nvim(&self, pane: &str) -> bool {
        let output = Command::new(&self.binary)
            .args(["pane", "process-info", "--pane", pane])
            .output()
            .ok()
            .map(|answered| String::from_utf8_lossy(&answered.stdout).into_owned())
            .unwrap_or_default();
        is_nvim_foreground(&output)
    }

    /// Carry out the decision.
    pub fn execute(&self, action: &Action) {
        match action {
            Action::SendKeys { pane, direction } => {
                self.run(&["pane", "send-keys", pane, direction.chord()]);
            }
            Action::Focus { pane, direction } => {
                self.run(&[
                    "pane",
                    "focus",
                    "--direction",
                    direction.as_str(),
                    "--pane",
                    pane,
                ]);
            }
            Action::FocusCurrent { direction } => {
                self.run(&[
                    "pane",
                    "focus",
                    "--direction",
                    direction.as_str(),
                    "--current",
                ]);
            }
        }
    }

    /// THE EXIT STATUS IS DROPPED. This runs from a keybinding with no terminal
    /// attached and nobody reading a code, and herdr's own output already goes
    /// where the operator can see it. A press at the edge of the layout that
    /// moves nothing is the ordinary case, not a failure to report.
    fn run(&self, args: &[&str]) {
        let _ = Command::new(&self.binary).args(args).status();
    }
}
