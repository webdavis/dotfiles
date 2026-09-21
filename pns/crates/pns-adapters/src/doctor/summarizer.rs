/// What the configured summarizer answers to a two-word prompt, in one line.
///
/// THE ONE CHECK THAT TALKS TO THE REAL BINARY. The golden test pins the words
/// pns composes, so pns cannot drift on its own; this is what notices the day
/// a harness changes its own flags, because it runs them.
///
/// IT NEVER MOVES THE EXIT CODE, for the daemon line's reason: the doctor's
/// code is what an operator's automation reads as "notifications are broken",
/// and a summarizer that is down costs a paragraph rather than a card.
///
/// UNDER THE CONFIGURED DEADLINE, never a deadline of its own: the point is to
/// exercise what a recap would do, and a check that gave the backend longer
/// than the recap does would vouch for a run the recap never gets.
pub fn summarizer_report(
    settings: &pns_domain::recap::summarizer::Settings,
) -> pns_domain::doctor::Item {
    use pns_domain::doctor::{Item, Mark};
    let stated = settings.kind.word();
    let Some(invocation) = settings.invocation() else {
        return Item::note(format!(
            "{PREFIX}no summarizer is configured, so a recap has no summary"
        ));
    };
    match crate::run_summarizer(&invocation, settings.deadline, PROMPT) {
        Ok(answer) => Item::row(
            Mark::Good,
            format!(
                "{PREFIX}the {stated} summarizer answered: {}",
                pns_domain::render::clipped(&crate::flattened(&answer), ANSWER_MAX_CHARS)
            ),
        ),
        Err(failure) => Item::row(
            Mark::Warn,
            format!("{PREFIX}{}", failure.line(stated, settings.deadline)),
        ),
    }
}

/// TWO WORDS, because the check is whether the invocation reaches a model at
/// all: a prompt that asked for work would pay for tokens to learn the same
/// thing.
const PROMPT: &str = "Reply with exactly: ok";

/// How much of the answer the row prints. One line, and a model that narrated
/// its reasoning is still legible as having answered.
const ANSWER_MAX_CHARS: usize = 80;

const PREFIX: &str = "pns doctor: ";
