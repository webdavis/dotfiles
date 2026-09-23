//! Which model writes the recap's summary, and the exact words pns runs to
//! reach it.
//!
//! THE HARNESSES ARE FIRST CLASS AND `custom` IS THE ESCAPE HATCH. An operator
//! naming `claude` states a harness rather than an argument vector, so a flag
//! that moves is pns's to follow; `custom` is the vector itself, for a backend
//! pns has never heard of.
//!
//! COMPOSITION IS POLICY AND LIVES HERE, which is what lets the golden test
//! pin every known type's vector as a plain call with no process in it. The
//! adapter beside it spawns what this composed, adding only the stripped Codex
//! home's path when the invocation asks for it.

use std::time::Duration;

/// Which backend writes the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    Claude,
    Codex,
    Ollama,
    Hermes,
    /// The operator's own argument vector, in `command`.
    #[default]
    Custom,
}

impl Kind {
    /// The word a config file writes, which is also what the doctor row and
    /// the document's `source` field say.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Codex => "codex",
            Kind::Ollama => "ollama",
            Kind::Hermes => "hermes",
            Kind::Custom => "custom",
        }
    }
    /// The kind one word names, or None for a word that names none.
    pub fn of(word: &str) -> Option<Kind> {
        WORDS.iter().copied().find(|kind| kind.word() == word)
    }
    /// Whether this harness has a flag that sets its reasoning effort.
    pub fn takes_effort(self) -> bool {
        matches!(self, Kind::Claude | Kind::Codex)
    }
}

/// Every word `type` accepts, in the order a refusal lists them.
pub const WORDS: &[Kind] = &[
    Kind::Claude,
    Kind::Codex,
    Kind::Ollama,
    Kind::Hermes,
    Kind::Custom,
];

/// `[recap.summarizer]`, whole.
///
/// ONE NAMED VALUE rather than a row of arguments, for `Recap`'s own reason:
/// three of these are strings and two are counts, and adjacent in a call a
/// swap would have nothing to catch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub kind: Kind,
    pub command: Vec<String>,
    pub model: String,
    /// The reasoning effort, passed in the harness's own flag. Empty passes
    /// nothing and leaves the backend's default.
    pub effort: String,
    pub deadline: Duration,
    pub transcripts: bool,
    pub transcript_bytes_per_session: usize,
    pub transcript_bytes_total: usize,
    /// The instruction that replaces the fixed one, inline or in a file. Both
    /// set is refused at load.
    pub prompt: String,
    pub prompt_file: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            kind: Kind::Custom,
            command: Vec::new(),
            model: String::new(),
            effort: String::new(),
            deadline: DEFAULT_DEADLINE,
            transcripts: false,
            transcript_bytes_per_session: DEFAULT_BYTES_PER_SESSION,
            transcript_bytes_total: DEFAULT_BYTES_TOTAL,
            prompt: String::new(),
            prompt_file: String::new(),
        }
    }
}

/// How the prompt reaches the backend.
///
/// TWO PLACES BECAUSE THE BACKENDS HAVE TWO. `claude -p`, `codex exec` and
/// `ollama run` all read the prompt on standard input, and `hermes chat` takes
/// it as the value of `-q` and reads no pipe at all (MEASURED 2026-09-20 on
/// hermes 0.17.0), so a seam that only knew how to pipe could not reach it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub argv: Vec<String>,
    /// Whether the prompt is appended to `argv` rather than written to stdin.
    pub prompt_in_argv: bool,
    /// Whether the adapter runs this in pns's stripped Codex home: ephemeral,
    /// read-only and with no hooks, the way the turn summarizer runs.
    pub stripped_codex_home: bool,
}

impl Settings {
    /// Whether this machine has a summarizer at all. A `custom` with no
    /// command is the shipped default and means none, the way an unset key
    /// meant none before the table existed.
    pub fn configured(&self) -> bool {
        self.kind != Kind::Custom || !self.command.is_empty()
    }

    /// The words pns runs, or None when no summarizer is configured.
    ///
    /// VERIFIED AGAINST THE INSTALLED TOOLS on 2026-09-20: `claude -p` and
    /// `codex exec` both take the prompt on stdin and print the answer alone
    /// on stdout (codex's preamble is stderr); `ollama run`'s three flags are
    /// what keep thinking, word wrapping and terminal control bytes out of the
    /// answer; and `hermes chat -Q -t ""` prints the answer alone, with its
    /// session line on stderr and its toolset empty so the query runs no tool.
    ///
    /// NEITHER HARNESS RUNS THE OPERATOR'S HOOKS, or a summary would fire
    /// pns's own Stop hook about itself. VERIFIED 2026-09-22 on claude 2.1.280
    /// and codex 0.156.0: `claude --safe-mode` ran no settings hook where the
    /// same run without it ran two, `--tools ""` leaves it no tool, and codex
    /// printed `reasoning effort: low` for `-c model_reasoning_effort="low"`.
    pub fn invocation(&self) -> Option<Invocation> {
        let model = |flag: &str| match self.model.is_empty() {
            true => Vec::new(),
            false => vec![flag.to_string(), self.model.clone()],
        };
        let effort = |flag: &str, value: String| match self.effort.is_empty() {
            true => Vec::new(),
            false => vec![flag.to_string(), value],
        };
        let words = |fixed: &[&str], tail: Vec<String>| {
            let mut argv: Vec<String> = fixed.iter().map(|word| (*word).to_string()).collect();
            argv.extend(tail);
            argv
        };
        Some(match self.kind {
            Kind::Claude => Invocation {
                argv: words(
                    &["claude", "-p", "--safe-mode", "--tools", ""],
                    [model("--model"), effort("--effort", self.effort.clone())].concat(),
                ),
                prompt_in_argv: false,
                stripped_codex_home: false,
            },
            Kind::Codex => Invocation {
                argv: words(
                    &["codex", "exec", "--color", "never", "--skip-git-repo-check"],
                    [
                        model("-m"),
                        effort("-c", format!("model_reasoning_effort=\"{}\"", self.effort)),
                    ]
                    .concat(),
                ),
                prompt_in_argv: false,
                stripped_codex_home: true,
            },
            Kind::Ollama => Invocation {
                argv: words(
                    &["ollama", "run"],
                    vec![
                        self.model.clone(),
                        "--hidethinking".to_string(),
                        "--nowordwrap".to_string(),
                        "--think=false".to_string(),
                    ],
                ),
                prompt_in_argv: false,
                stripped_codex_home: false,
            },
            Kind::Hermes => Invocation {
                argv: words(&["hermes", "chat", "-Q", "-t", ""], {
                    let mut tail = model("-m");
                    tail.push("-q".to_string());
                    tail
                }),
                prompt_in_argv: true,
                stripped_codex_home: false,
            },
            Kind::Custom if self.command.is_empty() => return None,
            Kind::Custom => Invocation {
                argv: self.command.clone(),
                prompt_in_argv: false,
                stripped_codex_home: false,
            },
        })
    }
}

/// How long the summarizer may take before the recap gives up on it. Four
/// minutes, measured: see `Recap`'s own note on what it covers.
const DEFAULT_DEADLINE: Duration = Duration::from_secs(240);

/// How much of one session's transcript `--with-transcripts` may append, and
/// how much of the whole window's.
///
/// EIGHT AND SIXTY-FOUR KIBIBYTES, the design's own figures: enough for the
/// last few turns of a session and far short of a context nobody can afford.
const DEFAULT_BYTES_PER_SESSION: usize = 8 * 1024;
const DEFAULT_BYTES_TOTAL: usize = 64 * 1024;

#[cfg(test)]
#[path = "summarizer/tests.rs"]
mod tests;

/// The summary as every output form carries it: the paragraph, when it was
/// written, and which summarizer wrote it.
///
/// A FAILURE IS A SUMMARY WITH ONE LINE IN IT rather than an absent section,
/// which is what makes the failure visible on the page, in the document and on
/// a delivered card without three renderers each remembering to say so. The
/// mechanical sections are composed from the store and the source commands and
/// are untouched either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub lines: Vec<String>,
    /// The local time the paragraph was written, which a stored summary is
    /// printed with.
    pub written_at: String,
    /// The summarizer's own type word.
    pub source: String,
    pub failed: bool,
}
