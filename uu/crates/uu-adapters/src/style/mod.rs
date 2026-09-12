//! How uu looks on a terminal. THE ONLY PLACE IN UU THAT EMITS AN ESCAPE
//! SEQUENCE.
//!
//! DELIBERATELY UU'S OWN COPY. pns has a module that looks much like this one,
//! and the two must never become one crate: uu and pns ship as separate
//! projects that people install independently, so a shared dependency would
//! make either uninstallable on its own. Two tools agreeing on a look is worth
//! the duplication; two tools sharing a package is not.
//!
//! THE PALETTE IS GUM'S, which is the same choice the apply scripts made in
//! `.chezmoitemplates/cli-print-style-lib.sh.tmpl`. An operator reading an
//! apply and an operator reading a doctor should be reading one machine.

use std::io::IsTerminal;

/// Gum's signature pink, bold. The tool's own name, and nothing else.
const ACCENT: &str = "\u{1b}[1;38;5;212m";
/// Teal, bold. A section heading, one level below the tool.
///
/// A SECOND COLOUR RATHER THAN A SECOND INDENT, because the headings sit flush
/// left with the tool name above them and only the colour separates the levels.
const SECTION: &str = "\u{1b}[1;38;5;43m";
/// The same teal without the weight, for a heading's rule.
const SECTION_QUIET: &str = "\u{1b}[38;5;43m";
/// Green, for a row that works.
const GOOD: &str = "\u{1b}[38;5;42m";
/// Red, for a row that does not.
const BAD: &str = "\u{1b}[1;38;5;203m";
/// Amber, for a row that works less than fully.
const WARN: &str = "\u{1b}[38;5;221m";
/// Dimmed, for text that explains rather than reports.
const FAINT: &str = "\u{1b}[38;5;244m";
const RESET: &str = "\u{1b}[0m";

/// The widest a rule is ever drawn. A CEILING RATHER THAN A SIZE: body text
/// stops being readable long before a wide terminal runs out of columns.
const WIDEST: usize = 60;
/// The narrowest rule still worth drawing.
const NARROWEST: usize = 20;

/// The terminal's width, measured once per run so a resize cannot tear a
/// report already half printed.
static COLUMNS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// How wide a rule is drawn, for this run.
pub fn width() -> usize {
    *COLUMNS.get_or_init(|| clamped(terminal_columns()))
}

/// What a measurement means, held apart from taking one so it can be tested.
///
/// A PIPE OR A FILE GETS THE CEILING, deterministically: redirected output
/// should not depend on the size of whatever window launched the process.
fn clamped(measured: Option<usize>) -> usize {
    measured.map_or(WIDEST, |columns| columns.clamp(NARROWEST, WIDEST))
}

/// The columns of the terminal on stdout, or `None` when it is not one.
///
/// `TIOCGWINSZ` rather than `$COLUMNS`, because the environment variable is set
/// by interactive shells for themselves and is stale or absent in a launchd
/// job, which is where uu actually runs.
fn terminal_columns() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `size` is a valid, fully initialized `winsize` for the duration
    // of the call, and the kernel only writes into it.
    let answered = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &raw mut size) };
    // A WIDTH OF ZERO IS "IT DID NOT SAY", which some terminals answer with,
    // and clamping zero would silently draw every rule at the floor.
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
/// A PROCESS-WIDE ANSWER, because the flag is a process-wide question. Passing
/// it down to each command instead would be a parameter every future printing
/// command had to remember to accept, and the one that forgot would ignore the
/// flag silently.
static FORCED_PLAIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Record what argv said about colour. Called ONCE, by the dispatcher.
pub fn remember_forced_plain(forced_plain: bool) {
    let _ = FORCED_PLAIN.set(forced_plain);
}

/// Whether output is being painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Color,
    Plain,
}

impl Paint {
    /// What to use, given the flag the operator passed and where the output is
    /// going.
    ///
    /// FOUR WAYS TO TURN IT OFF AND ONE TO LEAVE IT ON, deliberately. Colour
    /// that reaches a file or a pipe is corruption rather than decoration, so
    /// every signal that this is not a terminal wins; the explicit flag wins
    /// over all of them, because an operator who typed it has already decided.
    pub fn decide(forced_plain: bool, destination_is_terminal: bool) -> Self {
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
    pub fn for_stdout() -> Self {
        Self::decide(
            *FORCED_PLAIN.get().unwrap_or(&false),
            std::io::stdout().is_terminal(),
        )
    }

    pub fn for_stderr() -> Self {
        Self::decide(
            *FORCED_PLAIN.get().unwrap_or(&false),
            std::io::stderr().is_terminal(),
        )
    }

    /// `text` in `color`, or `text` alone when nothing is being painted.
    fn wrap(self, color: &str, text: &str) -> String {
        match self {
            Self::Color => format!("{color}{text}{RESET}"),
            Self::Plain => text.to_string(),
        }
    }

    pub fn accent(self, text: &str) -> String {
        self.wrap(ACCENT, text)
    }

    pub fn faint(self, text: &str) -> String {
        self.wrap(FAINT, text)
    }
}

/// How one row reads at a glance, in presentation terms.
///
/// Nothing here knows what a lane or a webhook is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
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

/// One labelled line of a header.
///
/// THE LABEL IS THE POINT. Without one the reader has to guess what role the
/// text plays, and the guess that costs the most is "something went wrong".
pub struct HeaderLine<'a> {
    pub label: &'a str,
    pub text: &'a str,
}

/// The opening of a report: the command that produced it, labelled lines saying
/// what the reader is about to read, then a rule.
///
/// NO BOX. A box needs four sides to line up, so any terminal narrower than its
/// content mangles it. A rule cannot be mangled, because it has one side.
///
/// IT OPENS WITH A BLANK LINE, so the report does not begin flush against the
/// prompt the operator just typed.
pub fn header(paint: Paint, invocation: &str, lines: &[HeaderLine<'_>]) -> Vec<String> {
    let width = lines.iter().map(|line| line.label.len()).max().unwrap_or(0);
    let mut out = vec![String::new(), paint.accent(invocation)];
    out.extend(
        lines
            .iter()
            .map(|line| paint.faint(&format!("{:width$}   {}", line.label, line.text))),
    );
    out.push(rule(paint));
    out
}

/// A section heading: a diamond and the name in teal, then a rule carrying the
/// one line that says what the rows under it are for.
///
/// FLUSH LEFT, under a tool name that is also flush left. The rows below it are
/// what indent, so the eye finds the headings by scanning one column.
pub fn section(paint: Paint, title: &str, blurb: &str) -> String {
    let head = paint.wrap(SECTION, &format!("◆ {title}"));
    if blurb.is_empty() {
        // NO DANGLING RULE. A `──` with nothing after it reads as a sentence
        // that failed to print rather than as a heading with nothing to add.
        return head;
    }
    format!(
        "{head} {} {}",
        paint.wrap(SECTION_QUIET, "──"),
        paint.faint(blurb)
    )
}

/// A closing rule, the full width of the report.
pub fn rule(paint: Paint) -> String {
    paint.faint(&"─".repeat(width()))
}

/// One row under a section: its mark, then its text, indented.
pub fn row(paint: Paint, tone: Tone, text: &str) -> String {
    format!("  {} {text}", paint.wrap(tone.color(), "·"))
}

/// A row's continuation: no mark, dimmed, indented under the row it explains.
pub fn detail(paint: Paint, text: &str) -> String {
    paint.faint(&format!("    {text}"))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
