use std::process::Command;

use herdr_smart_nav_domain::Action;
use herdr_smart_nav_protocol::is_nvim_foreground;

pub struct Herdr {
    binary: String,
}

impl Herdr {
    pub fn new(binary: String) -> Self {
        Self { binary }
    }

    pub fn is_nvim(&self, pane: &str) -> bool {
        // The legacy probe trusts stdout even when herdr exits unsuccessfully.
        let output = Command::new(&self.binary)
            .args(["pane", "process-info", "--pane", pane])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        is_nvim_foreground(&output)
    }

    pub fn execute(&self, action: &Action) {
        match action {
            Action::SendKeys { pane, chord } => self.run(&["pane", "send-keys", pane, chord]),
            Action::Focus { pane, direction } => {
                self.run(&["pane", "focus", "--direction", direction, "--pane", pane]);
            }
            Action::FocusCurrent { direction } => {
                self.run(&["pane", "focus", "--direction", direction, "--current"]);
            }
        }
    }

    fn run(&self, args: &[&str]) {
        // Navigation keeps herdr's output but never propagates its exit status.
        let _ = Command::new(&self.binary).args(args).status();
    }
}
