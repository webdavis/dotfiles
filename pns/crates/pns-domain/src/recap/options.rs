/// The recap's two delivery switches, its volume threshold, and the command
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
/// `minimum_events` IS A KEY RATHER THAN A CONSTANT because nobody can calibrate it
/// yet: the locked volume threshold carries a tilde, and the machine it was
/// written for has no history to measure. The recap prints the window's real
/// count in its own header every time, so one week of real recaps settles the
/// number without a rebuild.
///
/// `summarizer` IS ITS OWN TABLE, `[recap.summarizer]`: a harness named by
/// word, or `custom` with the operator's own argument vector. A `custom` with
/// no command is the shipped default and means no summarizer, which is a
/// WORKING SETTING and the common one: the recap then posts the plain
/// mechanical lists and writes no summary.
///
/// `sources` AND `review_notes_glob` ARE THE SOURCES PNS CANNOT FIND ON ITS
/// OWN, which is why they are keys and why an absent one is the working
/// setting. pns owns no task tool, no repository list and no apply log, so
/// each list section is a COMMAND THE OPERATOR NAMES. UNSET MEANS THE SOURCE
/// IS NEVER READ AT ALL: no process is spawned and no directory is opened,
/// which is the fence that makes every one of those sections opt-in rather
/// than merely empty.
///
/// THE FOUR PERIODS AND `week_starts_on` ARE THE WINDOWS THEMSELVES, which is
/// why they sit here rather than being compiled in: whose morning starts at
/// 06:00 is the operator's own shape of a day. The four must tile it, and a
/// config that breaks that is refused at load with both windows named.
///
/// ONE NAMED VALUE, never a row of loose booleans. Several fields are bools or
/// counts; spread through a call they would sit adjacent and a swap would go
/// unnoticed, and named fields cannot be transposed. It is CLONE rather than
/// Copy only because the argv lists are `Vec`s, and the composition root
/// clones it once off a borrowed config.
#[derive(Debug, Clone, PartialEq)]
pub struct Recap {
    pub replay_card: bool,
    pub post_window_recap: bool,
    pub minimum_events: usize,
    /// How long the operator must have been away before a return earns a
    /// recap. Both this and `minimum_events` must hold.
    pub minimum_away: std::time::Duration,
    pub summarizer: super::summarizer::Settings,
    /// The windows whose summary the gateway writes in the background at each
    /// window's end. EMPTY IS THE WORKING SETTING: a summary is then written
    /// only when a recap is delivered or `--summarize` asks for one.
    pub pregenerate: Vec<String>,
    pub review_notes_glob: Option<String>,
    pub retain: std::time::Duration,
    pub sources: Sources,
    pub periods: super::window::Periods,
    pub week_starts_on: super::window::WeekStart,
    pub rows_per_section: usize,
}

/// The commands `[recap.sources]` names, one per list section. ARGV AND NEVER
/// A SHELL STRING, the way `summarizer` is: nothing is interpreted, so there
/// is no quoting rule and no injection surface, and `{since}` and `{until}`
/// are substituted into the words themselves.
///
/// EVERY ONE IS OPTIONAL AND UNSET IS OFF, which is what keeps a fresh install
/// from spawning a process nobody named.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    pub pull_requests: Option<Vec<String>>,
    pub commits: Option<Vec<String>>,
    pub tasks: Option<Vec<String>>,
    pub applies: Option<Vec<String>>,
}

impl Sources {
    /// Each section name beside the command that fills it, in page order.
    pub fn each(&self) -> [(&'static str, Option<&[String]>); 4] {
        [
            ("pull_requests", self.pull_requests.as_deref()),
            ("commits", self.commits.as_deref()),
            ("tasks", self.tasks.as_deref()),
            ("applies", self.applies.as_deref()),
        ]
    }
}

/// Every section name this engine can print, in the order the page prints
/// them. `--section` accepts these and refuses anything else.
pub const SECTION_NAMES: &[&str] = &[
    "agents",
    "pull_requests",
    "commits",
    "tasks",
    "applies",
    "review_notes",
    "open",
];

impl Default for Recap {
    fn default() -> Self {
        Recap {
            replay_card: true,
            post_window_recap: true,
            minimum_events: DEFAULT_MINIMUM_EVENTS,
            minimum_away: DEFAULT_MINIMUM_AWAY,
            summarizer: super::summarizer::Settings::default(),
            pregenerate: Vec::new(),
            review_notes_glob: None,
            retain: DEFAULT_RETAIN,
            sources: Sources::default(),
            periods: super::window::Periods::default(),
            week_starts_on: super::window::WeekStart::Monday,
            rows_per_section: DEFAULT_ROWS_PER_SECTION,
        }
    }
}

/// How many events a window needs before a recap is worth the operator's
/// attention. The operator's own stated figure; see `Recap`.
const DEFAULT_MINIMUM_EVENTS: usize = 8;

/// How long an absence must last before its return earns a recap. TWENTY
/// MINUTES, the operator's own figure: a coffee or a phone call is not an
/// absence worth a digest, however busy the agents were during it.
const DEFAULT_MINIMUM_AWAY: std::time::Duration = std::time::Duration::from_secs(20 * 60);

/// How long a row in the activity store is kept before the gateway's own
/// prune deletes it.
///
/// THIRTY DAYS, WRITTEN IN HOURS because the duration parser's vocabulary is
/// `<count><ms|s|m|h>`: a day is not one of its units, so the value a config
/// file carries for this key is spelled the same way every other duration in
/// pns is.
///
/// IT BOUNDS THE TABLE RATHER THAN THE RECAP. Every window the recap serves is
/// shorter than this, so the rows past it answer no question anybody asks, and
/// the sessions table keeps the name of a session whose events have gone.
///
/// ZERO IS REFUSED at the config, the way `[remind] delay` refuses it: a
/// retention of nothing deletes each row on the tick after it was written,
/// which reads as switching the store off and is not what a duration says.
const DEFAULT_RETAIN: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

/// How many rows one list section spends by default.
///
/// EIGHT, which is what the design locked. It is a display default rather than
/// a fetch cap: the command still answers with everything it has, and the
/// section's own remainder line says how many more there were.
const DEFAULT_ROWS_PER_SECTION: usize = 8;
