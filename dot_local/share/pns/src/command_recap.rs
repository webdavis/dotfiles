use crate::*;

/// The `recap` mode: one window of activity, rendered and posted, in a process
/// nobody is waiting on.
///
/// IT TAKES NO DECISION, which is what makes it a mode. The decision was taken
/// by the event that spawned it, and re-deciding here would be the second
/// reading of one moment `GateInputs` exists to forbid.
///
/// IT REACHES ONE DESTINATION, the durable route, and never the phone or the
/// banner. The phone layer was already delivered by the card that pointed here.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, in `quiet_mode`'s style rather than the
/// hook path's always-zero: this is hand-runnable, and a subcommand that
/// swallows a typo is a recap the operator believes was posted. The spawner
/// never reads the code.
pub(crate) fn recap_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED ON THE ROUTE AND ON THE SUMMARIZER, AND OPEN ON THE POST,
    // which is `pulse_mode`'s split: a config nobody can read named no route
    // and no command, so the recap goes to the default route, plainly, rather
    // than to a route the operator never asked for or through a program they
    // never named.
    let (hermes_key, recap) = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes").and_then(hermes_secret),
            config.recap,
        ),
        _ => (
            None,
            pns::config::Recap {
                digest_as_thread: false,
                ..Default::default()
            },
        ),
    };
    let entries = activity_in(since, until);
    // THE TWO EXTERNAL SOURCES ARE READ ONLY WHEN A KEY NAMES THEM, and both
    // are read HERE, in the process nobody is waiting on. A repository listing
    // is a network call somebody else's machine answers and a glob is a
    // directory read; neither belongs anywhere near the card, and neither is
    // allowed to cost the rest of the recap anything when it does not come
    // back.
    let fetched_merges =
        (!recap.repos.is_empty()).then(|| merged_pull_requests(&recap.repos, since, until));
    let fetched_notes = recap
        .review_notes
        .as_deref()
        .map(|pattern| notes_matching(pattern, &home, since, until));
    // ONE EPISODE, ONE BUDGET. The locked "the LLM runs once at the return
    // moment" is a moment rather than a call: this recap asks up to three
    // questions (the night, the merges, the notes) and `summarizer_deadline_secs`
    // is what the WHOLE episode may spend, so each call is bounded by what is
    // left of it. Per-call deadlines meant a 240-second key could hold two
    // processes for twelve minutes while the card had already said the recap
    // was in #pns, and a laptop that sleeps inside that window loses the recap
    // entirely. Adjudicated 2026-08-29.
    let episode = std::time::Instant::now() + Duration::from_secs(recap.summarizer_deadline_secs);
    // THE ANSWER IS TAKEN BEFORE THE BODY IS COMPOSED and nothing else waits on
    // it: this process was started so that a model could be slow somewhere
    // nobody is standing.
    // AND NOT OVER AN EMPTY WINDOW. A night with nothing in it has nothing to
    // select from, and the model would be handed "nothing was recorded in this
    // window" under an instruction to rewrite it as a timeline. That is a
    // process spawned to summarize nothing and an invitation to invent, on the
    // one path an operator reaches by hand.
    let answered = recap
        .summarizer
        .as_deref()
        .filter(|_| !entries.is_empty())
        .map(|argv| {
            summarize(
                argv,
                left_of(episode),
                &pns::recap::prompt(&entries, &|at| wall_clock(at)),
            )
        });
    let timeline = match &answered {
        None => pns::recap::Timeline::Mechanical,
        Some(None) => pns::recap::Timeline::Unanswered,
        Some(Some(lines)) => pns::recap::Timeline::Summarized(lines),
    };
    // ONE SUMMARIZER CALL PER SECTION, and each falls back on its own. They are
    // three different questions over three different sets of text, so one call
    // answering all three would need the backend to keep them apart in its
    // answer, and a section would then be lost to a separator a model got wrong
    // rather than to anything pns could see. THEY SHARE ONE DEADLINE, above: a
    // call reached with the episode's budget already spent is never started.
    let merge_lines = read_sources(&fetched_merges)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::merge_prompt));
    let note_lines = read_sources(&fetched_notes)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::note_prompt));
    let externals = pns::recap::Externals {
        merges: pns::recap::External {
            found: found(&fetched_merges),
            answered: merge_lines.as_deref(),
            truncated: truncated(&fetched_merges),
        },
        notes: pns::recap::External {
            found: found(&fetched_notes),
            answered: note_lines.as_deref(),
            truncated: truncated(&fetched_notes),
        },
    };
    let body = pns::recap::body(
        &entries,
        &wall_clock(Some(since)),
        &wall_clock(Some(until)),
        &|at| wall_clock(at),
        timeline,
        &externals,
    );
    post_recap(&body, recap.digest_as_thread, &home, hermes_key)
}
/// What the summarizer said about one external section, or None for every way
/// of not having an answer.
///
/// NOT OVER AN EMPTY SOURCE, which is `recap_mode`'s own rule about an empty
/// window applied a second time: a model handed nothing to select from is a
/// process spawned to summarize nothing and an invitation to invent.
fn summarized(
    recap: &pns::config::Recap,
    episode: std::time::Instant,
    sources: &[pns::recap::Sourced],
    prompt: fn(&[pns::recap::Sourced]) -> String,
) -> Option<Vec<String>> {
    summarize(
        recap.summarizer.as_deref()?,
        left_of(episode),
        &prompt(sources),
    )
}

/// What is left of the episode's one budget. Zero once it is spent, which
/// `summarize` reads as a call not worth starting.
fn left_of(episode: std::time::Instant) -> Duration {
    episode.saturating_duration_since(std::time::Instant::now())
}
/// The configured command, handed the window on stdin, and what it said back
/// as timeline lines. None for every way of not answering.
///
/// ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
/// key safe to hold anything: the words are the words, so there is no quoting
/// rule to get wrong and nothing in the window can be read as syntax.
///
/// THE SEAM IS THE ONE THE PROBES ALREADY USE. `run_bounded` writes the prompt
/// inside the deadline window, reads stdout lossily, kills the child when the
/// window closes and answers None on a non-zero exit, which is every rung of
/// this ladder but the last; `recap::answer` owns that one.
///
/// A BACKEND THAT IS NOT INSTALLED IS NOT A SPECIAL CASE. The spawn fails, the
/// seam answers None, and the recap posts the plain list saying so, which is
/// the same thing the operator sees when the model is simply slow.
///
/// AND NEITHER IS A SPENT BUDGET. An episode whose deadline is gone starts no
/// process at all: spawning one only to kill it on a zero-length window is a
/// model load nobody reads, and the plain list is already the answer.
fn summarize(argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
    if deadline.is_zero() {
        return None;
    }
    let (program, arguments) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);
    pns::recap::answer(&run_bounded(
        command,
        Some(prompt),
        deadline,
        pns::recap::MAX_ANSWER_BYTES as u64 + 1,
    )?)
}

/// The window bounds off argv, or None for anything this will not vouch for.
///
/// EVERY UNKNOWN WORD IS A REFUSAL, never a silent default: a recap over a
/// window nobody asked for is worse than none. Both bounds are required, both
/// are plain counts through the crate's one numeric gate, and a window that
/// runs backwards is refused rather than read as empty.
fn recap_bounds(arguments: &[String]) -> Option<(u64, u64)> {
    let mut since = None;
    let mut until = None;
    let mut tokens = arguments.iter();
    while let Some(token) = tokens.next() {
        let bound = match token.as_str() {
            "--since" => &mut since,
            "--until" => &mut until,
            _ => return None,
        };
        // A REPEATED FLAG IS A REFUSAL TOO: two windows were asked for and only
        // one can be answered.
        if bound.is_some() {
            return None;
        }
        *bound = Some(pns::parse_count(tokens.next()?)?);
    }
    match (since, until) {
        (Some(since), Some(until)) if since <= until => Some((since, until)),
        _ => None,
    }
}

/// One epoch as the operator's own wall clock reads it, or a placeholder of the
/// same width when there is no readable time. ONE FUNCTION for the header's two
/// bounds and every timeline line, so the recap cannot render two clocks.
fn wall_clock(epoch: Option<u64>) -> String {
    epoch
        .and_then(local_minutes_since_midnight)
        .map(|minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60))
        .unwrap_or_else(|| NO_WALL_CLOCK.to_string())
}
/// What a recap typed wrong is told.
const RECAP_USAGE: &str = "pns: usage: pns recap --since <epoch> --until <epoch>";

/// What a line shows for a moment whose clock could not be read: the same width
/// as a time, so the timeline still lines up.
const NO_WALL_CLOCK: &str = "--:--";

#[cfg(test)]
#[path = "command_recap/tests.rs"]
mod command_recap_tests;
