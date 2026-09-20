//! How morning looks on a terminal. THE ONLY PLACE IN MORNING THAT EMITS AN
//! ESCAPE SEQUENCE.
//!
//! Trimmed from pns's own `style` module (`pns/crates/pns-adapters/src/
//! style.rs`), the house vocabulary: gum's pink for the tool's name, a faint
//! rule under it, `◆ Title ──` section headings in their own color so a
//! reader can tell the tool from a section of it, and NO BOX. A box needs
//! four sides to line up, so any terminal narrower than its content mangles
//! it; a rule has one side and cannot be mangled.

use morning_domain::Line;
use std::io::IsTerminal;

/// Gum's signature pink, bold. The tool's own name, and nothing else.
const ACCENT: &str = "\u{1b}[1;38;5;212m";
/// Steel blue, bold. A section of the tool, kept apart from the tool's own pink.
const HEADING_COLOR: &str = "\u{1b}[1;38;5;75m";
/// Dimmed, for the rule, a held-back count and an empty section.
const FAINT: &str = "\u{1b}[38;5;244m";
/// Amber, for the one word that says a section could not be read.
const WARN: &str = "\u{1b}[38;5;221m";
const RESET: &str = "\u{1b}[0m";

/// The widest a rule or a row is ever drawn.
const WIDEST: usize = 60;
/// The narrowest still worth drawing rather than truncating to nothing.
const NARROWEST: usize = 20;
/// What a pipe or a file gets, deterministically, since it has no width to
/// report.
const UNMEASURED: usize = WIDEST;

/// Whether output is being painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Color,
    Plain,
}

/// The environment variable every well-behaved tool honours.
const NO_COLOR: &str = "NO_COLOR";
/// The house variable, shared with the apply scripts' style library, so one
/// setting makes the whole machine print plainly.
const HOUSE_PLAIN: &str = "REPORT_LIB_PLAIN";

impl Paint {
    /// What to use, given `--no-color` and whether stdout is a terminal.
    ///
    /// FOUR WAYS TO TURN IT OFF: the flag, a destination that is not a
    /// terminal, `NO_COLOR` set to anything non-empty, or `REPORT_LIB_PLAIN=1`.
    /// Any one of them wins, because color reaching a pipe or a file is
    /// corruption rather than decoration.
    pub fn decide(forced_plain: bool, destination_is_terminal: bool) -> Self {
        Self::decide_with_env(
            forced_plain,
            destination_is_terminal,
            std::env::var_os(NO_COLOR).as_deref(),
            std::env::var_os(HOUSE_PLAIN).as_deref(),
        )
    }

    fn decide_with_env(
        forced_plain: bool,
        destination_is_terminal: bool,
        no_color: Option<&std::ffi::OsStr>,
        house_plain: Option<&std::ffi::OsStr>,
    ) -> Self {
        if forced_plain
            || !destination_is_terminal
            || no_color.is_some_and(|value| !value.is_empty())
            || house_plain.is_some_and(|value| value == "1")
        {
            return Self::Plain;
        }
        Self::Color
    }

    fn wrap(self, color: &str, text: &str) -> String {
        match self {
            Self::Color => format!("{color}{text}{RESET}"),
            Self::Plain => text.to_string(),
        }
    }
}

/// The columns of the terminal on stdout, or `None` when it is not one.
///
/// `TIOCGWINSZ` rather than `$COLUMNS`, because the environment variable is
/// stale or absent in every context that matters here: a launchd job, a hook,
/// a subshell whose parent was resized. The ioctl asks the terminal.
fn terminal_columns() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `size` is a valid, fully initialized `winsize` for the duration
    // of the call, and the kernel only writes into it.
    let answered = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &raw mut size) };
    // A WIDTH OF ZERO IS "IT DID NOT SAY", which some terminals answer with.
    if answered != 0 || size.ws_col == 0 {
        return None;
    }
    Some(usize::from(size.ws_col))
}

/// What a measurement means, held apart from taking one so it can be tested.
fn clamped(measured: Option<usize>) -> usize {
    measured.map_or(UNMEASURED, |columns| columns.clamp(NARROWEST, WIDEST))
}

/// How wide the page is drawn, measured once for this call.
fn width() -> usize {
    clamped(terminal_columns())
}

/// A row cut to `width`, with a single ellipsis where it was cut.
fn clip(text: &str, width: usize) -> String {
    if text.chars().count() > width {
        let mut clipped: String = text.chars().take(width.saturating_sub(1)).collect();
        clipped.push('…');
        clipped
    } else {
        text.to_string()
    }
}

/// A section heading: a diamond, the title, and a faint rule.
fn heading_line(paint: Paint, title: &str) -> String {
    format!(
        "{} {}",
        paint.wrap(HEADING_COLOR, &format!("◆ {title}")),
        paint.wrap(FAINT, "──")
    )
}

/// Paints the page's shape into text.
///
/// Opens with a blank line so the page does not begin flush against the
/// prompt the operator just typed, and never ends on one: the last line
/// printed is always content.
pub fn render(paint: Paint, lines: &[Line]) -> String {
    render_at(paint, lines, width())
}

fn render_at(paint: Paint, lines: &[Line], width: usize) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            Line::Header(text) => {
                out.push('\n');
                out.push_str(&paint.wrap(ACCENT, text));
                out.push('\n');
                out.push_str(&paint.wrap(FAINT, &"─".repeat(width)));
                out.push('\n');
            }
            Line::Heading(title) => {
                out.push('\n');
                out.push_str(&heading_line(paint, title));
                out.push('\n');
            }
            Line::Row(text) => {
                out.push_str("  ");
                out.push_str(&clip(text, width.saturating_sub(2)));
                out.push('\n');
            }
            Line::More(count) => {
                out.push_str(&paint.wrap(FAINT, &format!("  ... {count} more")));
                out.push('\n');
            }
            Line::Nothing => {
                out.push_str(&paint.wrap(FAINT, "  nothing"));
                out.push('\n');
            }
            Line::Unavailable(reason) => {
                out.push_str("  ");
                out.push_str(&paint.wrap(WARN, "unavailable:"));
                out.push(' ');
                out.push_str(reason);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
