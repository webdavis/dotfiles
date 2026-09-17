use crate::*;
use pns_domain::github::poll::{Answer, PollState, advance};
use pns_domain::github::{GithubEvent, notifications::polled_event};

pub(crate) fn github_mode(verb: &str) -> i32 {
    let arguments: Vec<String> = crate::arguments_after_verb();
    match (verb, github_launch(&arguments)) {
        ("poll", Some(launch)) => github_poll(launch),
        // UNKNOWN IS AN ERROR, never a silent fallthrough, exactly as the
        // room sensor's verb is.
        _ => {
            eprintln!("{GITHUB_USAGE}");
            2
        }
    }
}

const GITHUB_USAGE: &str = "pns: usage: pns github poll [--daemon]";

/// Who launched a poll, which is the whole difference between a refusal worth
/// printing and one worth swallowing. `command_presence`'s own `Launch`, for
/// its reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Launch {
    Daemon,
    Operator,
}

use pns_application::GITHUB_DAEMON_FLAG;

/// Who launched this poll, or `None` for an argument tail this does not serve.
fn github_launch(arguments: &[String]) -> Option<Launch> {
    match arguments {
        [] => Some(Launch::Operator),
        [flag] if flag == GITHUB_DAEMON_FLAG => Some(Launch::Daemon),
        _ => None,
    }
}

/// `pns github poll`: one conditional request, and one submission per
/// notification nobody has seen.
///
/// THE STATE IS PUBLISHED LAST, which is what makes a failure lose nothing:
/// a poll that dies between the listing and the write repeats the whole batch
/// next tick, and the seen-set refuses whatever already landed. The other
/// order would advance the cursor over events that never reached a channel.
///
/// A CONFIGURATION REFUSAL IS LOUD ON EVERY PATH, unlike the room sensor's,
/// which is silent for the daemon. 401 is the one failure this whole feature
/// cannot be quiet about: a revoked token leaves a source that looks alive
/// and reports nothing, and there is no reading to go stale in its place. It
/// is bounded by the poll's own interval rather than by a second gate, which
/// at 60 seconds is one line a minute in a log the doctor reads.
fn github_poll(launch: Launch) -> i32 {
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return 0;
    };
    let (Ok(Some(source)), Some(now)) = (pns_adapters::parse_github(&config), now_secs()) else {
        return 0;
    };
    let state = state_dir();
    let stored = pns_adapters::read_poll_state(&state);
    let polled = pns_adapters::GithubNotifications::new(source.token).poll(&stored.last_modified);
    report(&stored, &state, polled, now, launch, &mut |event| {
        submitted(event, now)
    })
}

/// One answer, acted on: nothing at all, a batch to submit, or a complaint.
///
/// THE SUBMISSION IS INJECTED, and it is the only seam in this function. It
/// exists because the ordinary producer path reaches REAL DESTINATIONS, and a
/// test over this ordering must not: the recorder the tests hand in grades
/// what would have been submitted and in what order, while production hands
/// in the submission itself.
fn report(
    stored: &PollState,
    state: &std::path::Path,
    polled: pns_adapters::GithubPolled,
    now: u64,
    launch: Launch,
    submit: &mut impl FnMut(&GithubEvent),
) -> i32 {
    match polled {
        pns_adapters::GithubPolled::NotModified { interval_secs } => {
            // NOTHING HAPPENED, AND IT COST NO RATE LIMIT. Only the interval
            // moves, and only when the server asked for a different one, so a
            // quiet minute writes nothing at all.
            let (_, advanced) = advance(
                stored,
                &Answer {
                    identities: Vec::new(),
                    last_modified: String::new(),
                    interval_secs,
                },
                now,
            );
            published(state, stored, &advanced, launch)
        }
        pns_adapters::GithubPolled::Listed { threads, answer } => {
            let mapped: Vec<GithubEvent> = threads.iter().filter_map(polled_event).collect();
            let dropped = threads.len() - mapped.len();
            let (fresh, advanced) = advance(
                stored,
                &Answer {
                    identities: mapped.iter().map(|event| event.identity.clone()).collect(),
                    ..answer
                },
                now,
            );
            // THE FIRST POLL EVER SUBMITS NOTHING. `stored.last_modified` is
            // empty on no other tick: a published state always carries the
            // cursor the last 200 answered with. Notifications are never
            // marked read, so a fresh machine's first answer is the whole
            // unread backlog; this establishes the cursor and the seen-set
            // from it instead of paging the operator for everything at once.
            if !stored.last_modified.is_empty() {
                for identity in &fresh {
                    if let Some(event) = mapped.iter().find(|event| &event.identity == identity) {
                        submit(event);
                    }
                }
            }
            if dropped > 0 && launch == Launch::Operator {
                // COUNTED RATHER THAN NAMED. The reasons this build maps no
                // kind to are most of a busy account's notifications, so a
                // line per drop would be the noise the closed enum exists to
                // avoid; the count is what says the poll is working.
                println!("pns github: {dropped} notifications this build maps no kind to");
            }
            published(state, stored, &advanced, launch)
        }
        pns_adapters::GithubPolled::Unauthorized { status } => {
            eprintln!(
                "pns github: the notifications API answered {status}; \
                 `[plugins.github] token` names the vault entry to check. \
                 It must be a CLASSIC personal access token carrying the \
                 `notifications` scope, which is the only token these \
                 endpoints accept."
            );
            1
        }
        pns_adapters::GithubPolled::RateLimited => {
            // NOT THE SENTENCE ABOVE. The token is fine and nothing in the
            // config needs editing; the budget refills by itself.
            eprintln!("pns github: the rate limit is spent; this poll listed nothing");
            1
        }
        pns_adapters::GithubPolled::Unavailable { detail } => {
            // TRANSIENT BY ASSUMPTION, so it is silent under the daemon for
            // the room sensor's reason: a network that comes and goes would
            // otherwise be a line a minute about nothing anyone can fix.
            if launch == Launch::Operator {
                eprintln!("pns github: the notifications API could not be read ({detail})");
            }
            0
        }
    }
}

/// Publish the advanced state, unless nothing about it moved.
///
/// A WRITE THAT CHANGES NOTHING IS NOT MADE. The quiet case is one 304 a
/// minute forever, and rewriting the same file on each of them is a rename in
/// the state directory for no reason.
fn published(
    state: &std::path::Path,
    stored: &PollState,
    advanced: &PollState,
    launch: Launch,
) -> i32 {
    if advanced == stored {
        return 0;
    }
    if let Err(error) = pns_adapters::write_poll_state(state, advanced)
        && launch == Launch::Operator
    {
        eprintln!("pns github: the poll state could not be published ({error})");
    }
    0
}

/// One event, through the ordinary producer path.
///
/// THE PRODUCER API AND NOT A PATH OF ITS OWN. The envelope already carries
/// `extensions` for exactly this ("producer-specific data, carried verbatim
/// and never read here"), so the GitHub parts ride in `extensions.github` and
/// pns's own policy reads `signal`, `context` and `scope` the way it does for
/// every other producer. Nothing about the envelope changes.
///
/// THE REQUEST ID IS THE EVENT'S IDENTITY, which makes the delivery ledger a
/// second guard behind the seen-set: a repeat submission of one event is
/// answered as the existing record rather than delivered twice.
///
/// NO `class`, because GitHub is work rather than machine health: `priority`
/// is a posture page, a failed unattended upgrade or a dead daemon, and a
/// lint job is none of those.
fn submitted(event: &GithubEvent, now: u64) {
    if let Some(request) = request_for(event, now)
        && let Ok(encoded) = request.encode()
    {
        let _ = event_flow::submit_encoded(encoded.as_bytes());
    }
}

/// The envelope one event submits as, or nothing when the event cannot be
/// spelled as one.
///
/// SEPARATE FROM THE DISPATCH so what is submitted can be graded without
/// anything being delivered: the tests over this reach the same value the
/// ledger and the channel lookup do.
fn request_for(event: &GithubEvent, now: u64) -> Option<pns_protocol::Request> {
    let mut request = pns_protocol::Request::new(
        pns_protocol::RequestId::new(&event.identity).ok()?,
        pns_protocol::Name::new(pns_adapters::GITHUB).ok()?,
        pns_protocol::Name::new(EVENT_NAME).ok()?,
        // EVERY POLLED EVENT IS AN OBSERVATION, because the outcome a
        // notification carries is `Neutral`: it is GitHub telling pns that
        // something happened, not a turn waiting on the operator, so it
        // changes no workflow or marker state and arms no nag.
        pns_protocol::Signal::Observation,
    );
    // AN INSTANT THE PARSE COULD NOT READ FALLS BACK TO NOW rather than to
    // 1970, which every elapsed calculation downstream would read as work
    // that ran for half a century.
    request.occurred_at = Some(if event.occurred_at == 0 {
        now
    } else {
        event.occurred_at
    });
    request.detail = event.title.clone();
    // THE REPOSITORY IS THE PROJECT, full name and owner included, which is
    // the key `channel_for` tries first: one repository resolves to one
    // channel whichever producer named it, and an unmapped one reaches the
    // catch-all rather than nowhere.
    request.context.project = Some(event.repo.clone());
    request.extensions = pns_adapters::github_extensions(event);
    Some(request)
}

/// The `event` name every polled submission carries.
///
/// ONE NAME RATHER THAN THE KIND'S, because `event` is documented as the
/// source's own event name and is metadata: the kind is already in the
/// extension, where the decode reads it, and spelling it twice is two places
/// for it to disagree.
const EVENT_NAME: &str = "notification";

#[cfg(test)]
#[path = "command_github/tests.rs"]
mod github_tests;
