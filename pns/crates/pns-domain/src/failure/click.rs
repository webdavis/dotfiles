//! What a click on a failure banner opens.
//!
//! The banner is self-sufficient by design, because there is no supported way
//! to open more from a tap on a phone. At the desk there is: the banner already
//! runs a shell string on click, and for an event with no pane, which is every
//! delivery failure, that string is the no-op `:`. This is what goes in it.

/// How the full record is put in front of the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickView {
    /// A pane in the running herdr session. The operator is already looking at
    /// that window, so this is the smallest interruption.
    Herdr,
    /// A new terminal window.
    ///
    /// `open -na Ghostty.app --args -e ...` and NEVER `ghostty -e`: Ghostty's
    /// own `--help` says `-e` is unsupported on macOS, where the binary hands
    /// off to the running app rather than starting a session itself.
    Window,
    /// The operator's own command, with `{id}` replaced by the failure id.
    Command(String),
}

/// The placeholder an operator writes in their own command.
pub const ID_PLACEHOLDER: &str = "{id}";

/// The command a click runs, which is this and nothing else.
///
/// FIXED, not composed from the event. The banner's `-execute` string is
/// rendered by a shell, so anything interpolated into it is code; a failure id
/// is a number pns assigned and the rest is a literal, which is what keeps it
/// from becoming a place a producer can write.
pub fn click_command(id: u64) -> String {
    format!("pns click {id}")
}

impl ClickView {
    /// The view when the config names none.
    ///
    /// INFERRED RATHER THAN DEFAULTED TO ONE. An operator running herdr wants
    /// the pane; one who is not has no session for a pane to open in, and a
    /// configured default would be wrong for half of them on a fresh machine.
    pub fn inferred(herdr_present: bool) -> Self {
        if herdr_present {
            ClickView::Herdr
        } else {
            ClickView::Window
        }
    }

    /// The argv this view runs to show failure `id`.
    ///
    /// `None` for a `Command` whose string is empty, which is a view the
    /// operator asked for and did not finish writing: running an empty command
    /// would report success for a window that never opened.
    pub fn argv(&self, id: u64, herdr_path: &str, pns_path: &str) -> Option<Vec<String>> {
        let show = format!("{pns_path} failures {id}");
        match self {
            ClickView::Herdr => Some(vec![
                herdr_path.to_string(),
                "pane".to_string(),
                "split".to_string(),
                "--command".to_string(),
                show,
            ]),
            // `-na` is what forces a NEW window rather than raising whatever
            // Ghostty already has open, which would put the record behind
            // whatever the operator was reading.
            ClickView::Window => Some(vec![
                "/usr/bin/open".to_string(),
                "-na".to_string(),
                "Ghostty.app".to_string(),
                "--args".to_string(),
                "-e".to_string(),
                show,
            ]),
            ClickView::Command(command) if command.trim().is_empty() => None,
            // A CLICK RUNS IN A BARE LAUNCHD CONTEXT WITH NO PATH, so a
            // configured command needs absolute paths. Split on whitespace
            // rather than shelling out: a shell here would make the config
            // string executable code with the id inside it.
            ClickView::Command(command) => Some(
                command
                    .replace(ID_PLACEHOLDER, &id.to_string())
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
            ),
        }
    }
}

/// The configured view, or the inferred one when the name is empty.
///
/// An unknown name is REFUSED rather than falling back, because a typo that
/// quietly opened something else is a click the operator believes is
/// configured. `command` with no string is refused for the same reason.
pub fn parse_view(name: &str, command: &str, herdr_present: bool) -> Result<ClickView, String> {
    match name.trim() {
        "" => Ok(ClickView::inferred(herdr_present)),
        "herdr" => Ok(ClickView::Herdr),
        "window" => Ok(ClickView::Window),
        "command" if command.trim().is_empty() => Err(
            "[plugins.macos-banner] click_type is \"command\" but click_command is empty".into(),
        ),
        "command" => Ok(ClickView::Command(command.to_string())),
        other => Err(format!(
            "[plugins.macos-banner] click_type is {other:?}; the types are \
             \"herdr\", \"window\" and \"command\""
        )),
    }
}

#[cfg(test)]
#[path = "click/tests.rs"]
mod tests;
