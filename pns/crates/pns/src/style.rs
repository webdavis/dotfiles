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

/// The widest a frame or a rule is ever drawn.
///
/// A CEILING RATHER THAN A SIZE. Body text stops being readable long before a
/// wide terminal runs out of columns, so the report does not grow to fill a
/// full-screen window.
const WIDEST: usize = 60;

/// The narrowest frame still worth drawing.
///
/// "pns doctor" is ten characters, two borders and four columns of padding make
/// sixteen, so twenty leaves the title room to breathe. Under this the content
/// is truncated rather than the box being broken, because a cramped frame is
/// still a frame and a wrapped one is a pile of line noise.
const NARROWEST: usize = 20;

/// What a frame is drawn at when the destination has no width to report.
///
/// A PIPE OR A FILE GETS THE CEILING, deterministically. Redirected output
/// should not depend on the size of whatever window happened to launch the
/// process, and a test that captures a report needs one answer rather than the
/// runner's terminal.
const UNMEASURED: usize = WIDEST;

/// The terminal's width, measured once.
static COLUMNS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// How wide a frame and a closing rule are drawn, for this run.
///
/// MEASURED, THEN CLAMPED, and the measurement is what this used to get wrong.
/// A fixed sixty was defended here as giving every pane the same shape, which
/// it does right up until the pane is narrower than sixty: then the terminal
/// wraps `╭` and its rule onto a second line, and every border character lands
/// somewhere the box does not want it. A report that reflows is a small cost. A
/// report that is mangled is not readable at all.
///
/// Clamping to `WIDEST` keeps most of what the constant was for: every terminal
/// roomy enough gets the identical sixty-column report, so two panes side by
/// side still agree, and only a genuinely narrow one differs. Measured once per
/// run rather than per line, so a window resized mid-report cannot tear it.
pub(crate) fn width() -> usize {
    *COLUMNS.get_or_init(|| clamped(terminal_columns()))
}

/// What a measurement means, held apart from taking one so it can be tested.
fn clamped(measured: Option<usize>) -> usize {
    measured.map_or(UNMEASURED, |columns| columns.clamp(NARROWEST, WIDEST))
}

/// The columns of the terminal on stdout, or `None` when it is not one.
///
/// `TIOCGWINSZ` rather than `$COLUMNS`, because the environment variable is set
/// by interactive shells for themselves and is stale or absent in every context
/// that matters here: a launchd job, a hook, a subshell whose parent was
/// resized. The ioctl asks the terminal.
fn terminal_columns() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `size` is a valid, fully initialized `winsize` for the duration
    // of the call, and the kernel only writes into it.
    let answered = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &raw mut size) };
    // A WIDTH OF ZERO IS "IT DID NOT SAY", which some terminals answer with,
    // and clamping zero would silently draw every report at the floor.
    if answered != 0 || size.ws_col == 0 {
        return None;
    }
    Some(usize::from(size.ws_col))
}

/// The environment variable every well-behaved tool honours.
const NO_COLOR: &str = "NO_COLOR";

/// The house variable, shared with the apply scripts' style library, so one
/// setting makes the whole machine print plainly.
const HOUSE_PLAIN: &str = "REPORT_LIB_PLAIN";

/// Whether `--no-color` was typed, decided once when argv was read.
///
/// A PROCESS-WIDE ANSWER, because the flag is a process-wide question. An
/// operator who has decided about color has decided about the whole command,
/// not about one subcommand's report, so `pns --no-color doctor` and
/// `pns doctor --no-color` mean the same thing and every command that prints
/// reads the same answer. Threading a boolean from the dispatcher into each of
/// them instead would be a parameter every future printing command had to
/// remember to accept, and the one that forgot would ignore the flag silently.
///
/// `decide` below stays a pure function of its arguments, which is what the
/// tests exercise; this only supplies one of them.
static FORCED_PLAIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Record what argv said about color. Called ONCE, by the dispatcher.
pub(crate) fn remember_forced_plain(forced_plain: bool) {
    let _ = FORCED_PLAIN.set(forced_plain);
}

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
    pub(crate) fn for_stdout() -> Self {
        Self::decide(
            *FORCED_PLAIN.get().unwrap_or(&false),
            std::io::stdout().is_terminal(),
        )
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

/// The opening of a report: the command that produced it, then labelled lines
/// saying what the reader is about to read, then a rule.
///
/// NO BOX. A box needs four sides to line up, so any terminal narrower than its
/// content mangles it, and there is no width at which a box is safe and a plain
/// line is not. This module's own opening paragraph settled that before the
/// first frame was written: the house look is gum's pink over a faint rule with
/// NO BOX AROUND BODY TEXT. A rule cannot be mangled, because it has one side.
///
/// NOTHING HERE FLOATS EITHER. Every line after the command carries a label
/// naming its role, because a bare sentence under a command name reads like an
/// error rather than a description.
pub(crate) fn header(paint: Paint, command: &str, lines: &[String]) -> Vec<String> {
    let mut out = vec![paint.accent(command)];
    out.extend(lines.iter().map(|line| paint.faint(line)));
    out.push(rule(paint));
    // NO TRAILING BLANK. Every section already opens with one, so adding a
    // second here put two blank lines between the rule and the first heading.
    out
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
    paint.faint(&"─".repeat(width()))
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
