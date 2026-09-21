//! What `pns recap` was asked for, off argv alone.
//!
//! ARGV ONLY, NO CLOCK AND NO CONFIG, which is what makes every refusal here
//! a plain call in a test rather than a spawned process. Resolving a window
//! name to two epochs needs the local zone and the configured periods, so it
//! happens one layer up; what this owns is which of the four ways of naming a
//! span was used, and refusing two of them at once.

use pns_adapters::Wire;

/// One recap invocation, as typed.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Options {
    pub span: Span,
    pub sections: Vec<String>,
    pub limit: Option<usize>,
    /// `--json` or `--toon`, which selects the document AND its format.
    pub format: Option<Wire>,
    /// `--schema <file>` or `--schema -`, which selects the document in the
    /// mask's own format unless `format` says otherwise.
    pub schema: Option<String>,
    pub to: Option<String>,
    pub verbose: bool,
    /// The internal hand-off: this child's stdin carries the return card.
    pub card_on_stdin: bool,
}

/// How the window was named. THE FOUR SPELLINGS ARE ONE FIELD, which is what
/// makes "two span flags together" a refusal the type states rather than a
/// check somebody has to remember.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum Span {
    /// Nothing was named, so the window is whichever one ended most recently.
    #[default]
    MostRecentlyEnded,
    /// A window word, and whether `--previous` stepped it back.
    Named { window: String, previous: bool },
    /// `pns recap open`, which has no window and takes no window flag.
    Open,
    /// The `--since`/`--until` family, kept as typed for `recap_bounds`.
    Bounds(Vec<String>),
    /// `--duration <n>[m|h|d|w]`, which is `--since <n>` with `--until` now.
    Duration(String),
}

impl Span {
    fn is_default(&self) -> bool {
        *self == Span::MostRecentlyEnded
    }
}

/// The options one invocation names, or None for anything this will not vouch
/// for.
///
/// EVERY UNKNOWN WORD IS A REFUSAL, never a silent default, which is
/// `recap_bounds`' own rule: a recap over a window nobody asked for is worse
/// than none, and a subcommand that swallows a typo is a recap the operator
/// believes was posted.
pub(crate) fn options(arguments: &[String]) -> Option<Options> {
    let mut options = Options::default();
    let mut previous = false;
    let mut bounds: Vec<String> = Vec::new();
    let mut words = arguments.iter().peekable();
    while let Some(word) = words.next() {
        match word.as_str() {
            "-v" | "--verbose" => options.verbose = true,
            "--previous" => previous = true,
            pns_adapters::CARD_ON_STDIN => options.card_on_stdin = true,
            "--json" => options.format = Some(Wire::Json),
            "--toon" => options.format = Some(Wire::Toon),
            "--section" => options.sections.push(words.next()?.clone()),
            "--schema" => options.schema = Some(words.next()?.clone()),
            "--to" => options.to = Some(words.next()?.clone()),
            "--limit" => {
                options.limit = Some(
                    usize::try_from(pns_domain::count::parse_count(words.next()?)?)
                        .ok()
                        .filter(|limit| *limit > 0)?,
                );
            }
            "--duration" => {
                if !options.span.is_default() {
                    return None;
                }
                options.span = Span::Duration(words.next()?.clone());
            }
            "--since" | "--until" | "--since-epoch" | "--until-epoch" => {
                bounds.push(word.clone());
                bounds.push(words.next()?.clone());
            }
            OPEN => {
                if !options.span.is_default() {
                    return None;
                }
                options.span = Span::Open;
            }
            // A BARE WORD IS A WINDOW NAME OR NOTHING AT ALL. The name itself
            // is judged here so `pns recap yesterdya` is refused with the
            // accepted names listed rather than reaching the engine.
            name if pns_domain::recap::window::parse_window(name).is_some() => {
                if !options.span.is_default() {
                    return None;
                }
                options.span = Span::Named {
                    window: name.to_string(),
                    previous: false,
                };
            }
            _ => return None,
        }
    }
    // TWO SPAN FLAGS TOGETHER ARE A REFUSAL, in either order: a window name
    // and a `--since` are two windows, and only one can be answered.
    if !bounds.is_empty() {
        if !options.span.is_default() {
            return None;
        }
        options.span = Span::Bounds(bounds);
    }
    // `--previous` STEPS A WINDOW BACK, so with no window there is nothing
    // for it to step. Silently ignoring it would answer a different question
    // from the one that was asked.
    options.span = match (previous, options.span) {
        (true, Span::Named { window, .. }) => Span::Named {
            window,
            previous: true,
        },
        (true, _) => return None,
        (false, span) => span,
    };
    Some(options)
}

/// The verb that prints what is waiting on a person, with no window.
pub(crate) const OPEN: &str = "open";

#[cfg(test)]
#[path = "options/tests.rs"]
mod tests;
