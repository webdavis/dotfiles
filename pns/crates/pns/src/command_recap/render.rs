//! What one assembled recap is turned into: a page on the terminal, a
//! document for a consumer, or a message on a destination.
//!
//! THE ENGINE RUNS ONCE WHATEVER THE OUTPUT IS, which is what keeps the page
//! and the document a consumer diffs against it from disagreeing: the sources
//! are processes and files, so running them per form would be running them
//! twice.

use super::{Options, options::OPEN, request};
use crate::*;
use pns_adapters::{DURABLE, Wire, read_mask};
use pns_domain::recap::document::{Mask, apply, unknown_key};

/// Assemble the recap this invocation names, then put it where it was asked
/// for. The exit code is the refusal's, or the delivery's.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    options: &Options,
    recap: &pns_adapters::Recap,
    window: &super::window::Resolved,
    home: &str,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &pns_domain::routes::Routes,
) -> i32 {
    // A SECTION NOBODY CONFIGURED IS NOT A NAME `--section` ACCEPTS, which is
    // the design's own ruling: naming one would silently print a page with a
    // section missing, and the operator would look at their window rather
    // than at their config.
    if let Some(refusal) = unknown_section(options, recap) {
        eprintln!("pns: {refusal}");
        return 2;
    }
    let mask = match options.schema.as_deref().map(mask_of) {
        None => None,
        Some(None) => {
            eprintln!("pns: the field mask could not be read as JSON or as TOON");
            return 2;
        }
        Some(Some(mask)) => Some(mask),
    };
    // A STORE THAT CANNOT BE OPENED IS A REFUSAL, because a recap with no
    // agents section is not a recap. EVERY SPAN CHECKS THIS, `open` included:
    // its sessions come out of the same store, and a failed read there would
    // otherwise print a silent all-clear instead of the refusal.
    let store = pns_adapters::SqliteStore::for_records(state_dir());
    if store.activity_between(0, 0).is_err() {
        eprintln!("pns: the activity store could not be opened, so there is no recap to give");
        return 2;
    }
    // THE EPISODE'S END IS ONE INSTANT, SHARED WITH THE SUMMARY SECTION
    // BELOW. `assemble` spends `[recap.summarizer] deadline` on the timeline
    // and the review notes; the summary section is a second call over the
    // same budget, not a second full deadline of its own.
    let episode_ends_at = std::time::Instant::now() + recap.summarizer.deadline;
    let assembled = pns_application::BuildRecap {
        activity: &store,
        commands: &pns_adapters::ProcessSourceCommands,
        notes: &pns_adapters::ReviewNotes {
            home: home.to_string(),
        },
        summarizer: &pns_adapters::ProcessSummarizer,
    }
    .assemble(
        &request(options, recap, window),
        |at| pns_application::recap_wall_clock(at, local_minutes_since_midnight),
        move |_budget| move || episode_ends_at.saturating_duration_since(std::time::Instant::now()),
    );
    let mut assembled = assembled;
    let remaining = episode_ends_at.saturating_duration_since(std::time::Instant::now());
    super::summary::attach(&mut assembled, options, recap, window, &store, remaining);
    // A PREGENERATING RUN PRINTS NOTHING AND DELIVERS NOTHING. It is the
    // gateway writing this window's paragraph into the store ahead of whoever
    // reads it next, so rendering a page would be a page nobody is looking at.
    if options.pregenerate {
        return 0;
    }
    let clock =
        |at: Option<u64>| pns_application::recap_wall_clock(at, local_minutes_since_midnight);
    let externals = assembled.externals();
    let page = assembled.page(&externals, &clock);
    // `--schema` ALONE SELECTS THE DOCUMENT in the mask's own format, and
    // `--json` or `--toon` beside it overrides only the format.
    let wire = options.format.or(mask.as_ref().map(|(_, wire)| *wire));
    let body = match wire {
        Some(wire) => {
            let document = pns_application::document(&assembled, |at| {
                pns_adapters::local_timestamp(at).unwrap_or_default()
            });
            match mask.as_ref().map(|(mask, _)| mask) {
                None => wire.encode(&document),
                Some(mask) => match unknown_key(&document, mask, "") {
                    Some(key) => {
                        eprintln!(
                            "pns: the field mask names `{key}`, which is no field of a recap"
                        );
                        return 2;
                    }
                    None => wire.encode(&apply(&document, mask)),
                },
            }
        }
        None => match options.to.is_some() {
            // A DELIVERED PAGE IS FITTED TO THE MESSAGE BUDGET AND PLAIN, and
            // a printed one is neither: a terminal scrolls and has colour,
            // where a Discord message is one message with a character
            // ceiling and no escape codes.
            true => pns_domain::recap::sections::body(&page),
            false => styled(&page),
        },
    };
    match options.to.as_deref() {
        None => {
            println!("{body}");
            0
        }
        Some(OPEN) | Some("") => {
            eprintln!("pns: `--to` names no destination");
            2
        }
        Some(name) => deliver(name, &body, home, hermes_keys, discord, routes),
    }
}

/// The page in the house style: the accent colour on the title, `◆ Title ──`
/// headings, and the faint colour on whatever a heading says about itself.
///
/// NO BOX AND NO WIDTH CLAMP ON THE ROWS, which is `style`'s own rule: a rule
/// has one side and cannot be mangled by a terminal narrower than its
/// content, where a box has four. The budget's own character ceiling is a
/// DELIVERY bound and is deliberately not applied here.
///
/// PLAIN UNDER `NO_COLOR`, `--no-color` AND A PIPE, which `Paint::for_stdout`
/// already decides for every other page pns prints.
fn styled(page: &pns_domain::recap::sections::Page<'_>) -> String {
    let paint = pns_adapters::style::Paint::for_stdout();
    let mut out = Vec::new();
    for (index, section) in pns_domain::recap::sections::sections(page)
        .iter()
        .enumerate()
    {
        let (heading, said) = section.lines[0]
            .split_once(" (")
            .map(|(heading, said)| (heading, format!("({said}")))
            .unwrap_or((&section.lines[0], String::new()));
        match index {
            // THE FIRST SECTION IS THE WINDOW ITSELF, which is the page's
            // title rather than one of its headings.
            0 => out.push(paint.accent(&section.lines[0])),
            // TRIMMED WHEN THE HEADING SAYS NOTHING ABOUT ITSELF, because
            // `style::heading` always writes the separator and a blurb after
            // it, and a line ending in a space reads as one that was cut.
            _ => out.push(
                pns_adapters::style::heading(paint, heading, &said)
                    .trim_end()
                    .to_string(),
            ),
        }
        out.extend(section.lines[1..].iter().cloned());
    }
    out.join("\n")
}

/// The recap on one named destination, or the refusal listing the ones this
/// machine has.
fn deliver(
    name: &str,
    body: &str,
    home: &str,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &pns_domain::routes::Routes,
) -> i32 {
    let selection = pns_domain::registry::roster().all();
    let destinations = channel_dispatch::destinations(
        &selection,
        "",
        home,
        &Mobile::default(),
        hermes_keys,
        discord,
        routes,
    );
    if name != DURABLE && destinations.get(name).is_none() {
        let mut registered: Vec<&str> = vec![DURABLE];
        registered.extend(
            destinations
                .iter()
                .map(|destination| destination.id().as_str()),
        );
        eprintln!(
            "pns: `{name}` is no configured destination; this machine has {}",
            registered.join(", ")
        );
        return 2;
    }
    super::post(body, Some(name), home, hermes_keys, discord, routes)
}

/// The first `--section` this configuration does not serve, if any.
fn unknown_section(options: &Options, recap: &pns_adapters::Recap) -> Option<String> {
    let mut served: Vec<&str> = vec![
        pns_application::AGENTS_SECTION,
        pns_application::REVIEW_NOTES_SECTION,
        OPEN,
    ];
    served.extend(
        recap
            .sources
            .each()
            .into_iter()
            .filter(|(_, command)| command.is_some())
            .map(|(name, _)| name),
    );
    let named = options
        .sections
        .iter()
        .find(|section| !served.contains(&section.as_str()))?;
    served.sort_unstable();
    Some(format!(
        "`{named}` is no recap section on this machine; it serves {}",
        served.join(", ")
    ))
}

/// One field mask off a file or off standard input.
fn mask_of(source: &str) -> Option<(Mask, Wire)> {
    let text = match source {
        "-" => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).ok()?;
            text
        }
        path => std::fs::read_to_string(path).ok()?,
    };
    read_mask(&text)
}
