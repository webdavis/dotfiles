/// The condenser's verdict: the last `STATE|SUMMARY` line it printed.
///
/// THE SUMMARY HALF IS WHAT MAKES IT USABLE. A matched state with nothing
/// after the pipe used to count as a hit, which shipped a title-only
/// notification over a turn that had text (live 2026-08-12). A summary of
/// spaces renders as blank as no summary, so it must carry one non-blank
/// character or the whole line is a miss and the caller falls back.
pub fn condenser_verdict(codex_output: &str) -> Option<(String, String)> {
    codex_output
        .lines()
        .filter_map(|line| {
            let (state, summary) = line.split_once('|')?;
            matches!(state, "done" | "asking" | "blocked")
                .then(|| (state.to_string(), summary.to_string()))
        })
        .rfind(|(_, summary)| summary.chars().any(|character| !character.is_whitespace()))
}

/// The prompt the condenser answers. One line out, so the caller can parse it
/// without a model-shaped grammar.
///
/// `asking` IS NARROWED TO A QUESTION FOR THE HUMAN, not a turn that merely
/// mentions waiting. A live status line reading "waiting on the remaining
/// reviews, then I bring you the one checkpoint" was classified `asking` under
/// the looser wording (OBS-3), which lit the blocked lamp and carded the operator
/// over a turn asking them nothing: the word "waiting" was enough to match,
/// whoever the turn was waiting on. There is no keyword rule to fix; the
/// condenser is a model call, and this sentence is the whole rule it reads.
pub fn condenser_prompt(reply: &str) -> String {
    format!(
        "Summarize this AI coding agent's last turn for a brief phone notification, then classify it.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work, or is only reporting status while it waits on other agents or tools, with nothing needed from you), asking (has a question or choice for YOU, the human operator, to answer), blocked (needs your permission or input to proceed).
SUMMARY is two or three sentences, up to 320 characters, plain text, no newlines, covering what was done plus any decision or question raised.

Turn:
{reply}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_condensers_last_usable_line_wins() {
        assert_eq!(
            condenser_verdict("noise\ndone|first\nasking|second"),
            Some(("asking".to_string(), "second".to_string()))
        );
    }

    #[test]
    fn a_state_with_a_blank_summary_is_a_miss_not_a_hit() {
        // It used to count, which shipped a title-only notification over a
        // turn that had text.
        assert_eq!(condenser_verdict("done|"), None);
        assert_eq!(condenser_verdict("done|   "), None);
        assert_eq!(condenser_verdict(""), None);
        assert_eq!(condenser_verdict("just some prose"), None);
    }

    #[test]
    fn a_state_the_prompt_never_offered_is_not_a_verdict() {
        assert_eq!(condenser_verdict("finished|all good"), None);
    }

    #[test]
    fn the_prompt_carries_the_turn_and_asks_for_one_line() {
        let prompt = condenser_prompt("what happened");
        assert!(prompt.contains("EXACTLY one line"));
        assert!(prompt.ends_with("Turn:\nwhat happened"));
    }
}
