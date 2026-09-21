//! Whether this recap gets a summary, where it comes from, and what stands in
//! its place when it does not.
//!
//! THE MODEL IS HANDED THIS RECAP'S OWN DOCUMENT. The mechanical sections are
//! assembled first and encoded as schema 1, so the paragraph is written over
//! exactly what the consumer beside it reads, and `open` is composed whatever
//! the answer says.
//!
//! A FAILURE IS A SUMMARY WITH ONE LINE IN IT, never an absent section, which
//! is what puts the same visible sentence on the page, in the document and on
//! a delivered card without three renderers each remembering to.

use super::{Options, window::Resolved};
use crate::*;
use pns_domain::recap::summarizer::{Settings, Summary};

/// Attach this run's summary, if it is to have one.
///
/// A MACHINE WITH NO SUMMARIZER GETS NO SECTION AT ALL, which is the working
/// setting and the common one: the recap is the mechanical sections, as it was
/// before the table existed.
///
/// `deadline` IS WHAT IS LEFT OF THE EPISODE, not `settings.deadline` again:
/// the caller has already spent part of it on the timeline and the review
/// notes, and this is the same budget's last call rather than a second one.
pub(super) fn attach(
    assembled: &mut pns_application::Assembled,
    options: &Options,
    recap: &pns_adapters::Recap,
    window: &Resolved,
    store: &pns_adapters::SqliteStore,
    deadline: std::time::Duration,
) {
    let settings = &recap.summarizer;
    if !settings.configured() {
        return;
    }
    // NOT OVER AN EMPTY WINDOW, which is the timeline's own rule applied a
    // second time: a model handed a window with nothing in it is a process
    // spawned to summarize nothing and an invitation to invent.
    if assembled.last_event_at() == 0 {
        return;
    }
    let asked = options.summarize || options.pregenerate;
    let stored = window
        .name
        .as_deref()
        .and_then(|name| store.recap_summary(name).ok().flatten());
    // A STORED SUMMARY IS SHOWN WITH THE TIME IT WAS WRITTEN, unless it is
    // older than the window's last event or the caller asked for a new one.
    // THIS COSTS NO MODEL CALL, so it is shown on a bare terminal recap too,
    // not only a delivered one.
    if let Some(held) = stored.filter(|held| !asked && held.covers >= assembled.last_event_at()) {
        assembled.with_summary(Summary {
            lines: vec![held.text],
            written_at: pns_adapters::local_timestamp(held.at).unwrap_or_default(),
            source: held.source,
            failed: false,
        });
        return;
    }
    // WITHOUT BEING ASKED, A NEW SUMMARY IS WRITTEN ONLY WHEN THE RECAP IS
    // DELIVERED. `--summarize` asks for it on the terminal, and
    // `--pregenerate` is the gateway writing one in the background; a bare
    // terminal recap with nothing stored gets no section rather than a model
    // call it never asked for.
    if !asked && options.to.is_none() {
        return;
    }
    let now = now_secs().unwrap_or_default();
    let written_at = pns_adapters::local_timestamp(now).unwrap_or_default();
    let summary = match write(assembled, options, settings, deadline) {
        Ok(lines) => Summary {
            lines,
            written_at,
            source: settings.kind.word().to_string(),
            failed: false,
        },
        Err(line) => Summary {
            lines: vec![line],
            written_at,
            source: settings.kind.word().to_string(),
            failed: true,
        },
    };
    // ONLY A PREGENERATING RUN WRITES TO THE STORE, and only a paragraph: a
    // stored failure would be shown as this window's summary until the next
    // window ended, long after the backend was fixed.
    let stores = options.pregenerate && !summary.failed;
    if let Some(name) = window.name.as_deref().filter(|_| stores) {
        let _ = store.store_recap_summary(
            name,
            &pns_adapters::StoredSummary {
                at: now,
                covers: assembled.last_event_at(),
                source: summary.source.clone(),
                text: summary.lines.join(" "),
            },
        );
    }
    assembled.with_summary(summary);
}

/// The paragraph the summarizer wrote, or the one line saying why there is
/// none.
fn write(
    assembled: &pns_application::Assembled,
    options: &Options,
    settings: &Settings,
    deadline: std::time::Duration,
) -> Result<Vec<String>, String> {
    let invocation = settings
        .invocation()
        .ok_or_else(|| "no summarizer is configured".to_string())?;
    let instruction = instruction(settings)?;
    let document = pns_adapters::Wire::Json.encode(&pns_application::document(assembled, |at| {
        pns_adapters::local_timestamp(at).unwrap_or_default()
    }));
    let mut prompt = pns_domain::recap::prompt::summary_prompt(&instruction, &document);
    // THE EXCERPT REACHES THE MODEL AND NOTHING ELSE. It is appended to the
    // prompt here rather than to the document above, so no consumer of a recap
    // can be handed a transcript by asking for one.
    if options.with_transcripts || settings.transcripts {
        prompt.push_str(&pns_adapters::transcript_excerpt(
            assembled.projects(),
            settings.transcript_bytes_per_session,
            settings.transcript_bytes_total,
        ));
    }
    let answered = pns_adapters::run_summarizer(&invocation, deadline, &prompt)
        .map_err(|failure| failure.line(settings.kind.word(), deadline))?;
    pns_domain::recap::prompt::summary_answer(&answered)
        .ok_or_else(|| pns_adapters::SummarizerFailure::Silent.line(settings.kind.word(), deadline))
}

/// The instruction ahead of the document: the operator's own, or the one pns
/// ships.
///
/// A `prompt_file` THAT WILL NOT OPEN IS A FAILURE LINE rather than a silent
/// fall back to the shipped instruction, which would summarize under words the
/// operator believes they replaced.
fn instruction(settings: &Settings) -> Result<String, String> {
    if !settings.prompt.is_empty() {
        return Ok(settings.prompt.clone());
    }
    if settings.prompt_file.is_empty() {
        return Ok(pns_domain::recap::prompt::SUMMARY_INSTRUCTION.to_string());
    }
    std::fs::read_to_string(&settings.prompt_file).map_err(|_| {
        format!(
            "the summarizer's `prompt_file` ({}) could not be read",
            settings.prompt_file
        )
    })
}
