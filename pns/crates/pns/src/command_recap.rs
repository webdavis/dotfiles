use crate::*;
use pns_application::{RECAP_USAGE, RecapRequest};

mod options;
mod render;
mod summary;
pub(crate) mod window;
pub(crate) use options::PREGENERATE;
use options::{OPEN, Options, Span, options};

/// The `recap` mode: one window of activity, rendered and posted, or printed
/// where the operator typed it.
///
/// IT TAKES NO DECISION, which is what makes it a mode. The decision was taken
/// by the event that spawned it, or by the operator, and re-deciding here would
/// be the second reading of one moment `GateInputs` exists to forbid.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, in `mute_mode`'s style rather than the
/// hook path's always-zero: this is hand-runnable, and a subcommand that
/// swallows a typo is a recap the operator believes was posted. The spawner
/// never reads the code.
///
/// THREE VERBS AND EVERY WINDOW. The window forms are the engine's; `agent`
/// posts a recap an agent wrote to the durable route; `git` prints the part of
/// that recap only git, worktrunk and `gh` can answer, so the skill pastes it
/// instead of composing it by hand.
///
/// ONLY A DELIVERED RECAP RUNS UNDER THE GROUP WATCHDOG. That deadline exists
/// for a child NOBODY IS WATCHING; a recap printed to a terminal has an
/// operator in front of it who can interrupt, and every spawn it makes is
/// bounded on its own.
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
/// window's recap takes.
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
    let (hermes_keys, discord, _, _, routes, durable) = recap_settings(&home);
    post(&body, durable, &home, &hermes_keys, &discord, &routes)
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
    let arguments = crate::arguments_after_subcommand();
    let Some(options) = options(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let card = options.card_on_stdin.then(handed_card).flatten();
    let home = std::env::var("HOME").unwrap_or_default();
    let (hermes_keys, discord, mobile, recap, routes, durable) = recap_settings(&home);
    let options = Options {
        to: durable_named(options.to, durable),
        ..options
    };
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
    // A RECAP WITH NO CLOCK IS A REFUSAL rather than a window over epoch
    // zero, which would report a quiet day in 1970.
    let Some(now) = now_secs() else {
        eprintln!("pns: the system clock could not be read, so no window can be stated");
        return 2;
    };
    match window::resolve(&options, &recap, now) {
        Err(refusal) => {
            eprintln!("pns: {refusal}");
            eprintln!("{RECAP_USAGE}");
            2
        }
        Ok(window) => render::run(
            &options,
            &recap,
            &window,
            &home,
            &hermes_keys,
            &discord,
            &routes,
        ),
    }
}

/// The request the engine is handed, from one resolved window.
pub(crate) fn request<'request>(
    options: &'request Options,
    recap: &'request pns_adapters::Recap,
    window: &'request window::Resolved,
) -> RecapRequest<'request> {
    RecapRequest {
        recap,
        window: window.name.clone(),
        previous: window.previous,
        since: window.since,
        until: window.until,
        sections: match options.span {
            Span::Open => vec![OPEN.to_string()],
            _ => options.sections.clone(),
        },
        verbose: options.verbose,
        limit: options.limit,
        windowed: options.span != Span::Open,
    }
}

/// The card this child was handed, read off its stdin.
///
/// STDIN IS READ ONLY WHEN THE FLAG SAID SO. An operator running this form by
/// hand passes no flag, so nothing here ever reads their terminal.
fn handed_card() -> Option<pns_adapters::HandedCard> {
    let mut line = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut line).ok()?;
    pns_adapters::decode_handed_card(&line)
}

/// The durable destinations' credentials, the recap's own settings and the
/// log transport the config selects, or the fail-closed reading.
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
    Option<&'static str>,
) {
    let loaded = load_config(&config_path(home));
    let (hermes_keys, discord, mobile, recap, routes) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(config, "hermes")
                .map(hermes_keys)
                .unwrap_or_default(),
            read_discord(config),
            read_mobile(config),
            config.recap.clone(),
            config.routes.clone(),
        ),
        _ => (
            HermesKeys::default(),
            DiscordSettings::default(),
            Mobile::default(),
            pns_adapters::Recap::default(),
            pns_domain::routes::Routes::default(),
        ),
    };
    let (selection, _) = select_plugins(&roster(), loaded);
    (
        hermes_keys,
        discord,
        mobile,
        recap,
        routes,
        selection.durable_log(),
    )
}

/// `--to durable` as the log transport `[plugins.log] type` selects on this
/// machine, and every other name as typed.
fn durable_named(to: Option<String>, durable: Option<&str>) -> Option<String> {
    match (to.as_deref(), durable) {
        (Some(pns_adapters::DURABLE), Some(name)) => Some(name.to_string()),
        _ => to,
    }
}

/// One composed body on the durable route. ONE POSTER FOR BOTH RECAPS, the
/// engine's and the agent's, so neither can drift onto a route of its own.
pub(crate) fn post(
    body: &str,
    to: Option<&str>,
    home: &str,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &pns_domain::routes::Routes,
) -> i32 {
    pns_application::post_return_recap(body, |body, route| {
        crate::recap_delivery_runtime::deliver_recap(
            body,
            route,
            to,
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

/// Every window word one refusal lists.
pub(crate) fn window_words() -> String {
    pns_domain::recap::window::WINDOW_WORDS.join(", ")
}

/// The verb that posts a recap somebody else composed.
const AGENT: &str = "agent";
/// The verb that prints what only git and `gh` can answer.
const GIT: &str = "git";
/// Where `agent` reads the recap from, named rather than assumed.
const STDIN: &str = "--stdin";
