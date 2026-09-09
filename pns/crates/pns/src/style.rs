//! How pns looks on a terminal. THE ONLY PLACE IN PNS THAT EMITS AN ESCAPE
//! SEQUENCE.
//!
//! This is the house vocabulary rather than one command's decoration: a
//! palette, a rounded frame, a section rule and a set of marks, so every pns
//! command that grows a report reads as the same tool. The doctor is the first
//! caller; anything else that prints more than a sentence should reach for
//! these rather than invent a second look.
//!
//! THE PALETTE IS GUM'S. This repository already picked it once, for the apply
//! scripts' own style library at
//! `.chezmoitemplates/cli-print-style-lib.sh.tmpl`, and settled on gum's
//! signature pink over a faint rule with no box around body text. The frame
//! here is gum's rounded border for the same reason: an operator reading an
//! apply and an operator reading a doctor should be reading one machine.

use std::io::IsTerminal;

/// Gum's signature pink, bold. The accent, used sparingly: a frame, a heading,
/// and nothing else.
const ACCENT: &str = "\u{1b}[1;38;5;212m";
/// The same pink without the weight, for a rule that should not shout.
const ACCENT_QUIET: &str = "\u{1b}[38;5;212m";
/// Green, for a row that works.
const GOOD: &str = "\u{1b}[38;5;42m";
/// Red, for a row that does not.
const BAD: &str = "\u{1b}[1;38;5;203m";
/// Amber, for a row that works less than fully.
const WARN: &str = "\u{1b}[38;5;221m";
/// Dimmed, for text that explains rather than reports.
const FAINT: &str = "\u{1b}[38;5;244m";
const RESET: &str = "\u{1b}[0m";

/// How wide a frame and a closing rule are drawn.
///
/// A CONSTANT, NOT THE TERMINAL'S WIDTH. Reading the width gives a report that
/// reflows differently in every pane and wraps differently in a recording; a
/// fixed frame is the same shape everywhere, and 60 columns fits the narrowest
/// pane anyone splits this machine into.
pub(crate) const WIDTH: usize = 60;

/// The environment variable every well-behaved tool honours.
const NO_COLOR: &str = "NO_COLOR";

/// The house variable, shared with the apply scripts' style library, so one
/// setting makes the whole machine print plainly.
const HOUSE_PLAIN: &str = "REPORT_LIB_PLAIN";

/// Whether output is being painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paint {
    Color,
    Plain,
}

impl Paint {
    /// What to use, given the flag the operator passed and where the output is
    /// going.
    ///
    /// FOUR WAYS TO TURN IT OFF AND ONE TO LEAVE IT ON, deliberately. Color
    /// that reaches a file or a pipe is corruption rather than decoration, so
    /// every signal that this is not a terminal wins; the explicit flag wins
    /// over all of them, because an operator who typed it has already decided.
    pub(crate) fn decide(forced_plain: bool, destination_is_terminal: bool) -> Self {
        if forced_plain
            || !destination_is_terminal
            || std::env::var_os(NO_COLOR).is_some_and(|value| !value.is_empty())
            || std::env::var_os(HOUSE_PLAIN).is_some_and(|value| value == "1")
        {
            return Self::Plain;
        }
        Self::Color
    }

    /// What printing to this process's own output means right now.
    pub(crate) fn for_stdout(forced_plain: bool) -> Self {
        Self::decide(forced_plain, std::io::stdout().is_terminal())
    }

    /// `text` in `color`, or `text` alone when nothing is being painted.
    fn wrap(self, color: &str, text: &str) -> String {
        match self {
            Self::Color => format!("{color}{text}{RESET}"),
            Self::Plain => text.to_string(),
        }
    }

    pub(crate) fn accent(self, text: &str) -> String {
        self.wrap(ACCENT, text)
    }

    pub(crate) fn faint(self, text: &str) -> String {
        self.wrap(FAINT, text)
    }

    pub(crate) fn good(self, text: &str) -> String {
        self.wrap(GOOD, text)
    }

    pub(crate) fn bad(self, text: &str) -> String {
        self.wrap(BAD, text)
    }
}

/// How one row reads at a glance, in presentation terms.
///
/// The report's own `Mark` maps onto this; nothing here knows what a channel or
/// a route is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Good,
    Bad,
    Warn,
    Quiet,
}

impl Tone {
    fn color(self) -> &'static str {
        match self {
            Self::Good => GOOD,
            Self::Bad => BAD,
            Self::Warn => WARN,
            Self::Quiet => FAINT,
        }
    }
}

/// A gum-style rounded frame around one or more lines.
///
/// GLYPHS SURVIVE PLAIN MODE and only the color is dropped, here and at every
/// mark below. Plain mode is for a pipe or a file, where an escape sequence is
/// corruption; the shape of the report is not decoration, and stripping it
/// would leave the plain output saying less than the painted output rather than
/// the same thing unpainted.
pub(crate) fn frame(paint: Paint, lines: &[(String, bool)]) -> Vec<String> {
    let inner = WIDTH - 2;
    let mut out = vec![paint.accent(&format!("╭{}╮", "─".repeat(inner)))];
    out.push(edge(paint, &" ".repeat(inner)));
    for (text, is_title) in lines {
        let padded = pad(&format!("  {text}"), inner);
        let painted = if *is_title {
            paint.accent(&padded)
        } else {
            paint.faint(&padded)
        };
        out.push(edge(paint, &painted));
    }
    out.push(edge(paint, &" ".repeat(inner)));
    out.push(paint.accent(&format!("╰{}╯", "─".repeat(inner))));
    out
}

/// One framed line, with its two side rules.
fn edge(paint: Paint, body: &str) -> String {
    format!("{}{body}{}", paint.accent("│"), paint.accent("│"))
}

/// `text` padded to `width` DISPLAY characters, counted the way the frame is
/// drawn.
fn pad(text: &str, width: usize) -> String {
    let counted = text.chars().count();
    if counted >= width {
        return text.chars().take(width).collect();
    }
    format!("{text}{}", " ".repeat(width - counted))
}

/// A section heading: a diamond, the name, and a rule carrying the one line
/// that says what the rows under it are for.
pub(crate) fn heading(paint: Paint, title: &str, blurb: &str) -> String {
    format!(
        "{} {}",
        paint.accent(&format!("◆ {title}")),
        paint.wrap(ACCENT_QUIET, "──")
    ) + &format!(" {}", paint.faint(blurb))
}

/// A closing rule, the full width of the frame.
pub(crate) fn rule(paint: Paint) -> String {
    paint.faint(&"─".repeat(WIDTH))
}

/// One row: its mark, then its text.
pub(crate) fn row(paint: Paint, tone: Tone, glyph: &str, indent: usize, text: &str) -> String {
    format!(
        "{}{} {text}",
        " ".repeat(indent),
        paint.wrap(tone.color(), glyph)
    )
}

#[cfg(test)]
#[path = "style/tests.rs"]
mod tests;
