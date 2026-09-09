/// The recap's three delivery switches, its volume threshold, and the command
/// it hands the window to.
///
/// ABSENT IS ALL ON, which is what makes the table optional: a machine that
/// never writes one behaves exactly as it did before the table existed. Each
/// boolean gates ONLY its own delivery, so recap-only and card-only are both
/// valid configurations and neither implies the other.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived. `#[derive(Default)]` reads
/// a bool as false, which would take every delivery away from every machine
/// whose config was written before this table existed, and it would do it
/// silently.
///
/// `min_events` IS A KEY RATHER THAN A CONSTANT because nobody can calibrate it
/// yet: the locked volume threshold carries a tilde, and the machine it was
/// written for has no history to measure. The recap prints the window's real
/// count in its own header every time, so one week of real recaps settles the
/// number without a rebuild.
///
/// `summarizer` IS ARGV AND NEVER A SHELL STRING, which is what makes it a
/// backend switch rather than a plugin: nothing is interpreted, so there is no
/// quoting rule and no injection surface, and a different backend is simply a
/// different array. UNSET IS A WORKING SETTING, and the common one: with no
/// summarizer the recap posts the plain mechanical lists.
///
/// `repos` AND `review_notes` ARE THE TWO SOURCES PNS CANNOT FIND ON ITS OWN,
/// which is why they are keys and why an absent one is the working setting.
/// The engine knows project NAMES off a working directory and nothing about
/// which repository they are, and the review notes are one operator's own
/// pipeline directory. UNSET MEANS THE SOURCE IS NEVER READ AT ALL: no `gh` is
/// spawned and no directory is opened, which is the fence that makes both
/// sections opt-in rather than merely empty.
///
/// ONE NAMED VALUE, never a row of loose booleans. Four of the eight fields are
/// bools or counts; spread through a call they would sit adjacent and a swap
/// would go unnoticed, and named fields cannot be transposed. It is CLONE
/// rather than Copy only because the argv is a `Vec`, and the composition root
/// clones it once off a borrowed config.
#[derive(Debug, Clone, PartialEq)]
pub struct Recap {
    pub replay_card: bool,
    pub digest: bool,
    pub digest_as_thread: bool,
    pub min_events: usize,
    pub summarizer: Option<Vec<String>>,
    pub summarizer_deadline_secs: u64,
    pub repos: Vec<String>,
    pub review_notes: Option<String>,
}

impl Default for Recap {
    fn default() -> Self {
        Recap {
            replay_card: true,
            digest: true,
            digest_as_thread: true,
            min_events: DEFAULT_MIN_EVENTS,
            summarizer: None,
            summarizer_deadline_secs: DEFAULT_SUMMARIZER_DEADLINE_SECS,
            repos: Vec::new(),
            review_notes: None,
        }
    }
}

/// How many events a window needs before a recap is worth the operator's
/// attention. The operator's own stated figure; see `Recap`.
const DEFAULT_MIN_EVENTS: usize = 8;

/// How long the summarizer may take before the recap gives up on it and posts
/// the plain lists.
///
/// FOUR MINUTES, and it is generous on purpose. WHAT IT COVERS IS GENERATION,
/// not a model load. Measured with `ollama run qwen3.5:4b` on one machine (an
/// M1 under load): a cold model load cost about 5.5 seconds, paid once, while
/// a full three-call episode took about 114.6 seconds, of which roughly 113.9
/// was tokens being generated at about eleven a second. Prefill was 185
/// milliseconds for 2,050 tokens, so the whole bill is the LENGTH OF THE
/// ANSWER and every other term rounds to noise. Nobody is waiting on it,
/// because the caller is the detached process the event path never joined.
///
/// THE SECONDS ARE ONE MACHINE ON ONE EVENING. What is durable is the shape
/// (prefill free, generation everything, the load small and paid once); the
/// figures are here to be recalibrated by whoever next tunes this number, and
/// no test encodes one. A backend that generates less is what makes this
/// faster, and the config file's own comment carries how.
///
/// ZERO IS ACCEPTED AND IS NOT A TRAP, unlike `min_events`'s zero. A deadline
/// of nothing simply cannot be met, so the recap falls to the plain lists and
/// SAYS it did, which is the same outcome as any other summarizer that does not
/// answer. Nothing silently changes shape, so there is nothing to refuse.
const DEFAULT_SUMMARIZER_DEADLINE_SECS: u64 = 240;
