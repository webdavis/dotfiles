use crate::style::{self, Paint, Tone};
use pns_adapters::{DEFAULT_HERMES_URL, DEFAULT_MOSHI_URL, SqliteStore};
use pns_application::StoredFailure;
use pns_domain::failure::{self, ClickView, Failure};

/// How many a bare `pns failures` lists. Twenty covers a bad night without
/// paging, and carrying the status and route on every line means most failures
/// are diagnosed from the list without opening one.
const LISTING_LIMIT: u32 = 20;

/// The `failures` mode: the detail view over what is not arriving.
///
/// IT NEVER PROMPTS, and that is a property of the shape rather than of a flag.
/// The design allows a picker and pairs it with an interaction model so a picker
/// could not block a script; a printed listing plus `pns failures <id>` needs no
/// picker, so there is nothing to guard and no tty to infer from. If a picker is
/// ever wanted, the interaction model arrives with it.
///
/// IT IS NOT ON THE EVENT PATH, so the always-exit-0 contract does not reach it.
/// A word it does not know is a refusal, the way `pns mute` and `pns remind`
/// refuse one: a subcommand that swallows a typo answers a question the operator
/// did not ask.
pub(crate) fn failures_mode() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_subcommand();
    let store = SqliteStore::for_records(pns_adapters::state_dir());
    match arguments.as_slice() {
        [] => list(&store),
        // A VERB BEFORE THE NUMBER PARSE, and the two can never collide: an id
        // is a number and a verb is a word.
        [word] if word == "serve" => serve(),
        [word] if word == DRAIN_VERB => drain(&store),
        [verb, word] if verb == OPEN_VERB => match word.parse::<u64>() {
            Ok(id) => open(id),
            Err(_) => {
                eprintln!("{FAILURES_USAGE}");
                2
            }
        },
        [word] => match word.parse::<u64>() {
            Ok(id) => show(&store, id),
            Err(_) => {
                eprintln!("{FAILURES_USAGE}");
                2
            }
        },
        _ => {
            eprintln!("{FAILURES_USAGE}");
            2
        }
    }
}

fn list(store: &SqliteStore) -> i32 {
    let Ok(failures) = store.failing_legs(LISTING_LIMIT) else {
        eprintln!("pns: the delivery ledger could not be read");
        return 1;
    };
    let paint = Paint::for_stdout();
    // THE CAP IS DISCLOSED, because a listing that silently stops at twenty
    // reads as "twenty things are failing" when the real answer may be more.
    // The page does not carry this line: it is about this invocation.
    for line in style::header(
        paint,
        "pns failures",
        &[style::HeaderLine {
            label: "Showing",
            text: &format!("the {LISTING_LIMIT} most recent failing legs"),
        }],
    ) {
        println!("{line}");
    }
    print!("{}", listing(paint, &failures));
    0
}

/// The listing, as text.
///
/// A STRING RATHER THAN A PRINT, because the page serves this same table and
/// the design's one rule about it is that there must not be two formatters that
/// can drift. The trailing pointer is part of it: whoever is holding the list is
/// exactly the reader who needs to know how to open one of its rows.
///
/// THE PAINT IS AN ARGUMENT FOR THE SAME REASON. The page serves this into a
/// browser's `<pre>`, where an escape sequence renders as literal line noise
/// rather than as colour, so it passes `Paint::Plain` while the terminal passes
/// whatever it resolved. Reading the destination in here would give the page the
/// terminal's answer.
pub(crate) fn listing(paint: Paint, failures: &[StoredFailure]) -> String {
    if failures.is_empty() {
        return "pns: nothing is failing to deliver\n".to_string();
    }
    // DERIVED, not a fixed guess: a fixed width is filled by whatever id
    // outgrows it, and ledger ids only grow (they are ledger_legs rowids).
    let id_width = failures
        .iter()
        .map(|f| f.id.to_string().len())
        .max()
        .unwrap_or(2)
        .max(2)
        + 1;
    let mut out = String::new();
    out.push_str(&style::heading(
        paint,
        "Not arriving",
        &plural(failures.len()),
    ));
    out.push('\n');
    // THE COLUMN HEADER IS FAINT, not a mark: it names the columns rather than
    // reporting anything, and a row's own glyph is what carries the verdict.
    out.push_str(&paint.faint(&format!(
        "    {:<id_width$}{:<18}{:<14}{:<11}sent by",
        "id", "when", "status", "route"
    )));
    out.push('\n');
    for failure in failures {
        out.push_str(&style::row(
            paint,
            Tone::Bad,
            "·",
            2,
            &format!(
                "{:<id_width$}{:<18}{:<14}{:<11}{}",
                failure.id,
                when(failure.failed_at),
                short_status(failure),
                failure.route,
                failure.agent
            ),
        ));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&paint.faint("run `pns failures <id>` for one in full"));
    out.push('\n');
    out
}

/// "3 legs" or "1 leg", for the heading's blurb.
fn plural(count: usize) -> String {
    if count == 1 {
        String::from("1 delivery leg")
    } else {
        format!("{count} delivery legs")
    }
}

/// `pns failures drain`: clears the legs nothing will ever deliver.
///
/// ONLY THE DEAD-LETTERED GO. A leg still inside its retry budget is left in
/// the listing because it may yet arrive; a leg the retry policy gave up on is
/// the one an operator is stuck with, and until this verb existed nothing could
/// take it off the list.
fn drain(store: &SqliteStore) -> i32 {
    let Ok(drained) = store.drain_deadlettered_legs() else {
        eprintln!("pns: the delivery ledger could not be written");
        return 1;
    };
    match drained {
        0 => println!("pns: nothing to drain"),
        1 => println!("pns: drained 1 dead-lettered leg"),
        count => println!("pns: drained {count} dead-lettered legs"),
    }
    0
}

/// `pns failures serve`: the page, in the foreground, until it is stopped.
///
/// THE DAEMON RUNS IT AS A CHILD, which is what supervises it: a listener that
/// dies is restarted on the next tick, and an operator who wants the page
/// without the daemon can still run this by hand.
///
/// `page_enabled = false` EXITS 0 RATHER THAN REFUSING. The daemon does not start this
/// child when the page is off, so reaching here with it off means the operator
/// typed the command themselves, and the honest answer is that the page is
/// switched off in their config rather than that they typed something wrong.
fn serve() -> i32 {
    let home = std::env::var("HOME").unwrap_or_default();
    let settings = match pns_adapters::load_config(&pns_adapters::config_path(&home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config.failures,
        _ => pns_adapters::Failures::default(),
    };
    if !settings.page_enabled {
        println!("pns: the failure page is off; set `[failures] page_enabled = true` to serve it");
        return 0;
    }
    // NEVER RETURNS while the daemon is up: the listener waits for its port
    // and then serves forever, so the exit below is what a stopped child gets.
    crate::failures_page::serve(settings.page_port);
    0
}

fn show(store: &SqliteStore, id: u64) -> i32 {
    match store.failing_leg(id) {
        Err(_) => {
            eprintln!("pns: the delivery ledger could not be read");
            1
        }
        // A leg that has since been acknowledged answers the same way one that
        // never existed does, because to the reader they are the same news:
        // that id is not something to chase.
        Ok(None) => {
            eprintln!("pns: no failure {id}; run `pns failures` for the current list");
            1
        }
        Ok(Some(stored)) => {
            let install =
                pns_adapters::install_settings(&std::env::var("HOME").unwrap_or_default());
            print!(
                "{}",
                failure::full(&compose(
                    &stored,
                    install.moshi_url.as_deref(),
                    install.hermes_url.as_deref()
                ))
            );
            0
        }
    }
}

/// The ledger's row plus the two facts only this layer holds: where the
/// destination lives, which comes from the config and the environment, and the
/// command the reader is shown.
///
/// `moshi_url` AND `hermes_url` ARE RESOLVED BY THE CALLER, once, rather than
/// read here: this runs once per failure row, and a caller looping over many
/// rows (the failure notice) would otherwise reparse the config file on every
/// one of them.
pub(crate) fn compose(
    stored: &StoredFailure,
    moshi_url: Option<&str>,
    hermes_url: Option<&str>,
) -> Failure {
    Failure {
        id: stored.id,
        destination: stored.destination.clone(),
        route: stored.route.clone(),
        address: address(&stored.destination, &stored.route, moshi_url, hermes_url),
        agent: stored.agent.clone(),
        // RECONSTRUCTED from the routing facts rather than stored. It is the
        // string the reader searches for, and pns knows exactly which flags
        // produced this leg; storing a second copy of them would be a column
        // that can disagree with the routing it describes.
        command: command(stored),
        outcome: stored.outcome,
        retries: stored.retries,
        max_attempts: pns_domain::retry::RetryLimits::default().max_retries,
    }
}

fn command(stored: &StoredFailure) -> String {
    let mut command = format!("pns send --producer {}", stored.agent);
    // Only a word `pns send --state` accepts belongs in a runnable command;
    // an internal word like "recap" would print a suggestion that refuses.
    if pns_protocol::State::from_word(&stored.state).is_some() {
        command.push_str(&format!(" --state {}", stored.state));
    }
    if !stored.route.is_empty() {
        command.push_str(&format!(" --route {}", stored.route));
    }
    command
}

/// Where the destination lives, as the reader would type it. Config
/// (`[plugins.phone] url` / `[plugins.log] url`) outranks the matching
/// variable (`PNS_MOSHI_URL` / `PNS_HERMES_URL`), which is what keeps the
/// message pointing at the gateway this machine actually posts to.
fn address(
    destination: &str,
    route: &str,
    moshi_url: Option<&str>,
    hermes_url: Option<&str>,
) -> String {
    if destination == failure::DESTINATION_PHONE {
        return moshi_url
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_MOSHI_URL.to_string());
    }
    let base = hermes_url
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_HERMES_URL.to_string());
    pns_adapters::channel_url(&base, route).unwrap_or(base)
}

/// The five-column listing's status, which is the full form's `status` without
/// the registered name: the name is what teaches, and a listing is what scans.
fn short_status(failure: &StoredFailure) -> String {
    match failure.outcome {
        pns_domain::retry::TransportOutcome::Status(code) => format!("HTTP {code}"),
        pns_domain::retry::TransportOutcome::NoResponse => "no response".to_string(),
        pns_domain::retry::TransportOutcome::NoStatus => "bad URL".to_string(),
    }
}

/// The epoch as the operator reads a clock, to the minute.
///
/// UTC and not the local zone, reusing the instant the recap window already
/// speaks. The column exists to order two failures against each other and to
/// say roughly when, which UTC does as well as local time, and it says `Z` so
/// nobody reads it as an hour it is not. The seconds are dropped because the
/// listing is scanned rather than parsed; `pns failures <id>` is where a reader
/// goes for precision.
fn when(epoch: u64) -> String {
    // `YYYY-MM-DDTHH:MM:SSZ`, cut at the minute and read back as a date and a
    // time. Cutting by length rather than by matching the seconds, because a
    // match would silently do nothing for 59 seconds out of every 60.
    let Some(instant) = pns_adapters::utc_timestamp(epoch) else {
        return "unknown".to_string();
    };
    let minute: String = instant.chars().take("YYYY-MM-DDTHH:MM".len()).collect();
    format!("{}Z", minute.replace('T', " "))
}

/// `pns failures open <id>`: what a click on a failure banner runs.
///
/// IT IS NOT TYPED BY THE OPERATOR, which is what shapes everything here. The
/// banner's `-execute` string calls it from a bare launchd context with no
/// PATH, no terminal and nobody watching stderr, so every path ends in
/// something the operator can SEE: a window, or a log line naming what was
/// tried.
fn open(id: u64) -> i32 {
    let herdr = crate::executable_in_path("herdr");
    let view = match view(herdr.is_some()) {
        Ok(view) => view,
        // A REFUSED CONFIG STILL OPENS THE RECORD. The operator clicked a
        // banner about a lost page; telling them their config is wrong and
        // showing them nothing answers a question they did not ask. The
        // complaint is printed and the fallback runs.
        Err(complaint) => {
            eprintln!("pns: {complaint}");
            ClickView::Window
        }
    };
    let outcome = pns_application::open_failure(
        &pns_adapters::SystemCommandRunner,
        &view,
        id,
        herdr.as_deref().unwrap_or("herdr"),
        &pns_path(),
    );
    if outcome.opened {
        return 0;
    }
    eprintln!("{}", pns_application::click_failure_line(id, &outcome));
    raise_last_resort_banner(id);
    1
}

/// The banner that reports a click nothing answered.
///
/// `":"` IS THE EXEC STRING, which is the no-op the channel already writes for
/// an event with no pane: the banner raises the terminal and runs nothing.
/// Clicking a failed click to be told the click failed is a loop, and this is
/// where it stops.
fn raise_last_resort_banner(id: u64) {
    let (title, message) = pns_application::click_banner(id);
    let args = pns_adapters::notifier_args(
        &title,
        &message,
        Some("default"),
        pns_adapters::DEFAULT_TERMINAL_BUNDLE_ID,
        ":",
    );
    // The outcome is DROPPED, and there is nowhere left to report it: a banner
    // that will not post is the third failure in a row, and the log line above
    // has already said everything a reader could act on.
    let _ = pns_application::CommandRunner::run(
        &pns_adapters::SystemCommandRunner,
        "terminal-notifier",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

fn view(herdr_present: bool) -> Result<ClickView, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(pns_adapters::LoadOutcome::Loaded(config)) =
        pns_adapters::load_config(&pns_adapters::config_path(&home))
    else {
        return Ok(ClickView::inferred(herdr_present));
    };
    match crate::plugin_settings(&config, "banner") {
        Some(settings) => pns_adapters::banner_click(settings, herdr_present),
        None => Ok(ClickView::inferred(herdr_present)),
    }
}

/// This binary's own path, so the view it opens runs THIS pns rather than
/// whatever a new shell's PATH resolves. A click has no PATH to resolve with,
/// and a machine mid-upgrade can have two.
pub(crate) fn pns_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(str::to_string))
        .unwrap_or_else(|| "pns".to_string())
}

/// What `pns click` answers now: the verb that replaced it, and nothing done.
pub(crate) fn retired_click() -> i32 {
    eprintln!("pns: click is now a verb: run `pns failures open <id>`");
    eprintln!("{FAILURES_USAGE}");
    2
}

/// The verb the banner's stored click command names.
const OPEN_VERB: &str = "open";

/// The verb that clears the dead-lettered legs.
const DRAIN_VERB: &str = "drain";

pub(crate) const FAILURES_USAGE: &str = "pns: usage: pns failures [<id>|open <id>|drain|serve]";

#[cfg(test)]
#[path = "command_failures/tests.rs"]
mod tests;
