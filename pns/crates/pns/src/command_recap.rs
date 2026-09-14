use crate::*;
use pns_application::{RECAP_USAGE, recap_bounds};

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
///
/// THREE VERBS, ONE ROUTE. The window form is the night's, spawned detached;
/// `agent` posts a recap an agent wrote to the same destination; `git` prints
/// the part of that recap only git, worktrunk and `gh` can answer, so the
/// skill pastes it instead of composing it by hand.
///
/// ONLY THE WINDOW FORM RUNS UNDER THE GROUP WATCHDOG. That deadline exists
/// for a child NOBODY IS WATCHING, and it is 30 seconds; the two agent verbs
/// have a caller waiting on them and every spawn each makes is bounded on its
/// own, so borrowing the watchdog would only cap a remote listing at less than the
/// listing is given.
pub(crate) fn recap_mode() -> i32 {
    match crate::arguments_after_subcommand()
        .first()
        .map(String::as_str)
    {
        Some(AGENT) => agent_recap(),
        Some(GIT) => git_recap(),
        _ => pns_adapters::run_recap_bounded(recap),
    }
}

/// The recap an agent wrote, fitted and posted to the same durable route the
/// night's recap takes.
///
/// `--stdin` IS REQUIRED RATHER THAN IMPLIED. A recap is a body somebody
/// composed, and a command that reads a terminal's stdin when no source was
/// named hangs on an operator who typed it to see what it does.
///
/// EMPTY IS A TYPO, NOT AN EMPTY RECAP. Nothing was piped in, or the pipe
/// broke; posting a blank message to the channel would report work nobody can
/// read.
fn agent_recap() -> i32 {
    if crate::arguments_after_verb() != [STDIN] {
        eprintln!("{RECAP_USAGE}");
        return 2;
    }
    let mut written = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut written).is_err() {
        eprintln!("{RECAP_USAGE}");
        return 2;
    }
    let body = pns_domain::recap::agent::fitted(&written);
    if body.trim().is_empty() {
        eprintln!("{RECAP_USAGE}");
        return 2;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let (hermes_key, recap) = recap_settings(&home);
    post(&body, &recap, &home, hermes_key)
}

/// The Git block, the stack graph and the file list, printed.
///
/// IT DELIVERS NOTHING. The skill pastes this into the recap it is composing,
/// and `pns recap agent --stdin` is what posts the finished one, so printing
/// here and posting there keeps one recap from being sent twice.
fn git_recap() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{RECAP_USAGE}");
        return 2;
    }
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    print!(
        "{}",
        pns_domain::recap::git_block::git_block(&pns_adapters::git_facts(&cwd))
    );
    0
}

fn recap() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_subcommand();
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let (hermes_key, recap) = recap_settings(&home);
    let body = pns_application::BuildReturnRecap {
        activity: &pns_adapters::SqliteStore::for_records(state_dir()),
        merges: &pns_adapters::GitHubMerges,
        notes: &pns_adapters::ReviewNotes { home: home.clone() },
        summarizer: &pns_adapters::ProcessSummarizer,
    }
    .run(
        &recap,
        since,
        until,
        |at| pns_application::recap_wall_clock(at, local_minutes_since_midnight),
        |budget| {
            let end = std::time::Instant::now() + budget;
            move || end.saturating_duration_since(std::time::Instant::now())
        },
    );
    post(&body, &recap, &home, hermes_key)
}

/// The hermes key and the recap's own settings, or the fail-closed reading.
///
/// FAIL CLOSED ON THE ROUTE AND ON THE SUMMARIZER, AND OPEN ON THE POST,
/// which is `pulse_mode`'s split: a config nobody can read named no route and
/// no command, so the recap goes to the default route, plainly, rather than to
/// a route the operator never asked for or through a program they never named.
fn recap_settings(home: &str) -> (Option<String>, pns_adapters::Recap) {
    match load_config(&config_path(home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes").and_then(hermes_secret),
            config.recap,
        ),
        _ => (
            None,
            pns_adapters::Recap {
                digest_as_thread: false,
                ..Default::default()
            },
        ),
    }
}

/// One composed body on the durable route, with the one fallback the spec
/// names. ONE POSTER FOR BOTH RECAPS, so `[recap] digest_as_thread` answers
/// the same question for each: whether a recap takes the `pns-recap` route or
/// the default one.
fn post(body: &str, recap: &pns_adapters::Recap, home: &str, hermes_key: Option<String>) -> i32 {
    pns_application::post_return_recap(body, recap.digest_as_thread, |body, route| {
        crate::recap_delivery_runtime::deliver_recap(body, route, home, hermes_key.clone())
            .into_iter()
            .map(|(_, outcome)| outcome)
            .collect()
    })
}

/// The verb that posts a recap somebody else composed.
const AGENT: &str = "agent";
/// The verb that prints what only git and `gh` can answer.
const GIT: &str = "git";
/// Where `agent` reads the recap from, named rather than assumed.
const STDIN: &str = "--stdin";
