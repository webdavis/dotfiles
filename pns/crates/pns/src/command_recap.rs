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
/// EXIT 2 FOR A MISTYPED INVOCATION, in `mute_mode`'s style rather than the
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
    let (hermes_keys, discord, _, _, routes) = recap_settings(&home);
    post(&body, &home, &hermes_keys, &discord, &routes)
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

/// The window form: the card the return moment handed this child, then the
/// recap it was spawned for.
///
/// THE CARD IS THIS PROCESS'S TO DISPATCH, which is what the hand-off moved
/// here: the recap is rendered and posted in this process, so this is the only
/// process that can ever put anything of the recap ON the card.
///
/// AND IT GOES FIRST, BEFORE THE SUMMARIZER IS EVER RUN. The two layers are
/// locked apart: the phone card is composed from the journal's own entries and
/// owes the summarizer nothing, so a model that never answers must not hold the
/// card up. Reading the pipe first is the same rule from the other side, since
/// the writer is the process that spawned this one and writes at once.
fn recap() -> i32 {
    let (arguments, card) = handed_card(crate::arguments_after_subcommand());
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let (hermes_keys, discord, mobile, recap, routes) = recap_settings(&home);
    if let Some(card) = card {
        crate::recap_delivery_runtime::deliver_recap_card(
            &card,
            &mobile,
            &home,
            &hermes_keys,
            &discord,
            &routes,
        );
    }
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
    post(&body, &home, &hermes_keys, &discord, &routes)
}

/// The card this child was handed, and the arguments with its flag removed.
///
/// THE FLAG IS STRIPPED BEFORE THE WINDOW IS PARSED rather than taught to the
/// parser, which keeps `recap_bounds` refusing every word it will not vouch
/// for and keeps an internal hand-off out of the operator's usage text.
///
/// STDIN IS READ ONLY WHEN THE FLAG SAID SO. An operator running this form by
/// hand passes no flag, so nothing here ever reads their terminal.
fn handed_card(arguments: Vec<String>) -> (Vec<String>, Option<pns_adapters::HandedCard>) {
    let (flags, arguments): (Vec<String>, Vec<String>) = arguments
        .into_iter()
        .partition(|argument| argument == pns_adapters::CARD_ON_STDIN);
    if flags.is_empty() {
        return (arguments, None);
    }
    let mut line = String::new();
    let read = std::io::Read::read_to_string(&mut std::io::stdin(), &mut line).is_ok();
    (
        arguments,
        read.then(|| pns_adapters::decode_handed_card(&line))
            .flatten(),
    )
}

/// The durable destinations' credentials and the recap's own settings, or the fail-closed reading.
///
/// FAIL CLOSED ON THE SUMMARIZER AND OPEN ON THE POST, which is
/// `lights_pulse`'s split: a config nobody can read named no command, so the
/// recap posts the plain mechanical lists rather than running a program the
/// operator never named.
fn recap_settings(
    home: &str,
) -> (
    HermesKeys,
    DiscordSettings,
    Mobile,
    pns_adapters::Recap,
    pns_domain::routes::Routes,
) {
    match load_config(&config_path(home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes")
                .map(hermes_keys)
                .unwrap_or_default(),
            read_discord(&config),
            read_mobile(&config),
            config.recap,
            config.routes,
        ),
        _ => (
            HermesKeys::default(),
            DiscordSettings::default(),
            Mobile::default(),
            pns_adapters::Recap::default(),
            pns_domain::routes::Routes::default(),
        ),
    }
}

/// One composed body on the durable route. ONE POSTER FOR BOTH RECAPS, the
/// night's and the agent's, so neither can drift onto a route of its own.
fn post(
    body: &str,
    home: &str,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &pns_domain::routes::Routes,
) -> i32 {
    pns_application::post_return_recap(body, |body, route| {
        crate::recap_delivery_runtime::deliver_recap(
            body,
            route,
            home,
            hermes_keys,
            discord,
            routes,
        )
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
