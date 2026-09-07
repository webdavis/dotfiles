use crate::*;

/// The `home` mode: one reading of the home probe, reported in one line, and
/// the one stale-identifier alert that reading may earn.
///
/// A DIAGNOSTIC FIRST: it always exits 0 and says what it found, including
/// every way it can be unconfigured, because its job is to answer "why did the
/// probe not read" as much as "is the device home". The key itself is never
/// printed, on any path.
///
/// AND THE TRIGGER for the stale-identifier alert, on exactly the condition
/// that prints the warning. This is the only code that reads the sensor and it
/// already holds the derive/decide/remember trio, so one call site keeps ONE
/// memory and ONE decision; a second entrypoint would be a second place for
/// the episode decision to fall out of step. The consequence is deliberate: a
/// hand-run `pns home` no longer consumes an episode silently, it delivers it.
pub(crate) fn home_mode() {
    use pns::home::{HomePresence, SetupFailure, report, setup_report};
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home_dir)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        Ok(LoadOutcome::Missing) => {
            println!("{}", setup_report(&SetupFailure::NoConfigFile));
            return;
        }
        Err(error) => {
            println!(
                "{}",
                setup_report(&SetupFailure::ConfigError(error.detail().to_string()))
            );
            return;
        }
    };
    // EVERY CAUSE IS DECIDED IN THE LIBRARY, so each line is pinned by a
    // value-in, value-out test and this stays wiring: a missing table, a
    // disabled one, a `type` nothing answers and a mistyped value each send
    // the operator to a different edit, and one message covering two of them
    // sends half of them to the wrong one.
    let router_table = match pns::home::enabled_router_table(&config) {
        Ok(table) => table,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // WHERE THE ALERT GOES, settled at the config read rather than at the
    // post. `hermes_url_for`'s own refusal names `--channel`, a flag nobody
    // typed on this path; this one names the key in the file, and it is said
    // on every run of the diagnostic instead of only on the run that happens
    // to have something to deliver.
    let (alert_route, complaint) = pns::home::stale_alert_channel(router_table);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    let settings = match pns::home::router_settings(router_table) {
        Ok(settings) => settings,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // The key stays its own read, so it never joins the settings in a type
    // that could be dumped whole.
    let Some(key) = pns::home::router_api_key(router_table) else {
        println!("{}", setup_report(&SetupFailure::NoApiKey));
        return;
    };
    let router = pns::home::UniFiRouter::new(settings.router_url, key);
    // STILL WIRING: the library decides what is stale, what its episode is
    // called and whether that is news; this reads the memory, prints, and
    // writes the memory back.
    let reading = pns::home::read_home(&router, &settings.device);
    // ONE DERIVATION, ONE DECISION. The episode is spelled once and the news
    // decided once, then the SAME value is what gets printed and what gets
    // remembered: two derivations of one fact, one in the print and one in
    // the write, can only stay in step for as long as neither grows a
    // condition of its own.
    let staleness = pns::home::stale_identifiers(&reading);
    let episode = staleness.as_ref().map(pns::home::episode_id);
    let news = pns::home::is_new_staleness(remembered_staleness().as_deref(), episode.as_deref());
    // ONE VALUE FEEDS BOTH SURFACES. The sentence the terminal prints and the
    // sentence the alert carries come out of this same Option, so there is no
    // second condition that could deliver what was not printed, or print what
    // was not delivered. It is Some only for a HOME reading with a
    // disagreement that is news, which is what keeps away, unreadable and
    // already-told runs silent without a guard of their own.
    let alert = staleness.as_ref().filter(|_| news);
    println!("{}", report(&reading, alert));
    // THE WARNING, DELIVERED. An ordinary event ABOUT the reading, handed to
    // the one event path: presence, surface and the leg plan decide where it
    // lands exactly as they do for a finished agent turn. Nothing narrows it
    // and it is not long-running, so it raises no pulse.
    //
    // DISPATCH BEFORE REMEMBER, AND THE ORDER IS LOAD-BEARING. Tidied into
    // remember-then-dispatch it would silently LOSE an alert: a crash, a
    // wedged channel or a kill between the two leaves the episode recorded
    // and never delivered, and the next run reads it as already told. This
    // way round the same interruption re-alerts instead, and two overlapping
    // hand runs that both read the memory before either writes both alert.
    // Duplicates are the direction to fail in.
    //
    // THE COST, ACCEPTED: the delivery OUTCOME is not consulted before the
    // write either, so a post the gateway rejected consumes the episode just
    // as a delivered one does. Fire-and-forget is this engine's contract for
    // every producer, and the printed line above has already told the one
    // human who typed the command.
    if let Some(staleness) = alert {
        run_event(
            &pns::args::EventArgs {
                agent: "pns".to_string(),
                state: "stale".to_string(),
                detail: pns::home::stale_warning(staleness),
                channel: alert_route,
                ..Default::default()
            },
            &system_probes(),
            &HookPayload::default(),
            Attempt::First,
        );
    }
    // ONLY A HOME READING HAS AN OPINION ABOUT THE IDENTIFIERS. NotHome and
    // Unknown both hand `stale_identifiers` a None, and writing that back
    // would read "the disagreement resolved" out of a trip to the shops or a
    // five-second router timeout: the same invention as reading a failed
    // fetch as NotHome, one layer up. Away and unreadable leave the memory
    // untouched, so the warning stays once per STATE rather than once per
    // homecoming.
    if matches!(reading.presence, HomePresence::Home { .. }) {
        remember_staleness(episode.as_deref());
    }
}
