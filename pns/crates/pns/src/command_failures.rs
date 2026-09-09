use pns_adapters::{DEFAULT_HERMES_URL, DEFAULT_MOSHI_URL, SqliteStore};
use pns_application::StoredFailure;
use pns_domain::failure::{self, Failure};

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
/// A word it does not know is a refusal, the way `pns quiet` and `pns nag`
/// refuse one: a subcommand that swallows a typo answers a question the operator
/// did not ask.
pub(crate) fn failures_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let store = SqliteStore::for_records(pns_adapters::state_dir());
    match arguments.as_slice() {
        [] => list(&store),
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
    if failures.is_empty() {
        println!("pns: nothing is failing to deliver");
        return 0;
    }
    println!(
        "  {:<4}{:<18}{:<14}{:<11}sent by",
        "id", "when", "status", "route"
    );
    for failure in &failures {
        println!(
            "  {:<4}{:<18}{:<14}{:<11}{}",
            failure.id,
            when(failure.failed_at),
            short_status(failure),
            failure.route,
            failure.agent
        );
    }
    println!();
    println!("run `pns failures <id>` for one in full");
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
            print!("{}", failure::full(&compose(&stored)));
            0
        }
    }
}

/// The ledger's row plus the two facts only this layer holds: where the
/// destination lives, which comes from the config and the environment, and the
/// command the reader is shown.
pub(crate) fn compose(stored: &StoredFailure) -> Failure {
    Failure {
        id: stored.id,
        destination: stored.destination.clone(),
        route: stored.route.clone(),
        address: address(&stored.destination, &stored.route),
        agent: stored.agent.clone(),
        // RECONSTRUCTED from the routing facts rather than stored. It is the
        // string the reader searches for, and pns knows exactly which flags
        // produced this leg; storing a second copy of them would be a column
        // that can disagree with the routing it describes.
        command: command(stored),
        outcome: stored.outcome,
        retries: stored.retries,
        max_attempts: pns_domain::retry::RetryLimits::default().max_attempts,
    }
}

fn command(stored: &StoredFailure) -> String {
    let mut command = format!("pns --agent {}", stored.agent);
    if !stored.state.is_empty() {
        command.push_str(&format!(" --state {}", stored.state));
    }
    if !stored.route.is_empty() {
        command.push_str(&format!(" --channel {}", stored.route));
    }
    command
}

/// Where the destination lives, as the reader would type it. The gateway
/// honours `PNS_HERMES_URL`, so reading it here is what keeps the message
/// pointing at the gateway this machine actually posts to.
fn address(destination: &str, route: &str) -> String {
    if destination == failure::DESTINATION_MOBILE {
        return std::env::var("PNS_MOSHI_URL").unwrap_or_else(|_| DEFAULT_MOSHI_URL.to_string());
    }
    let base = std::env::var("PNS_HERMES_URL").unwrap_or_else(|_| DEFAULT_HERMES_URL.to_string());
    pns_adapters::channel_url(&base, route).unwrap_or(base)
}

/// The five-column listing's status, which is the full form's `status` without
/// the registered name: the name is what teaches, and a listing is what scans.
fn short_status(failure: &StoredFailure) -> String {
    match failure.outcome {
        pns_domain::retry::DeliveryOutcome::Status(code) => format!("HTTP {code}"),
        pns_domain::retry::DeliveryOutcome::NoResponse => "no response".to_string(),
        pns_domain::retry::DeliveryOutcome::NoStatus => "bad URL".to_string(),
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

const FAILURES_USAGE: &str = "pns: usage: pns failures [<id>]";

#[cfg(test)]
#[path = "command_failures/tests.rs"]
mod tests;
