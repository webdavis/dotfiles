//! Turning what a child said into lines the record and one alert card can
//! carry.
//!
//! EVERY LINE HERE CROSSES A TRUST BOUNDARY. Child output reaches the record
//! and then Discord, so it is bounded in length, squashed onto one line, and
//! stripped of the backticks that would open or close a code fence around
//! every record line after it.

/// How much of a failed command's stderr a lane line carries.
pub const STDERR_TAIL: usize = 240;

/// How many of a command lane's last stdout lines the record keeps.
///
/// 20 LINES AT `STDERR_TAIL` (240) CHARACTERS EACH IS 4,800 CHARACTERS, chosen
/// against the Discord adapter that chunks a message at 2000 characters: a
/// talkative child at the cap spans about three of those messages rather than
/// dozens, so one command lane's stdout cannot crowd every other lane's line
/// out of the record.
pub(super) const STDOUT_LINES_KEPT: usize = 20;

/// The last `keep` characters of `text`, prefixed with `...` when it was cut.
/// Shared by `failure_reason`'s stderr tail and a command lane's own stdout
/// cap: BOUNDED because both go into the record and into one alert card, and
/// the verdict a tool prints is at the END of what it said.
pub fn tail(text: &str, keep: usize) -> String {
    let length = text.chars().count();
    if length <= keep {
        return text.to_string();
    }
    let cut = text
        .char_indices()
        .nth(length - keep)
        .map_or(text.len(), |(index, _)| index);
    format!("...{}", &text[cut..])
}

/// Why a command failed, in one line: how it ended, and the tail of what it
/// said about it.
///
/// THE STATUS ALONE IS NOT A REASON. `exit 1` sends the operator to a log that
/// a weekly job may have rotated away, while the command already printed the
/// answer on stderr and this is the last moment it exists. Squashed to a
/// single line so it is not reflowed by a build log.
pub fn failure_reason(how_it_ended: &str, stderr: &str) -> String {
    let said = stderr.trim();
    if said.is_empty() {
        return how_it_ended.to_string();
    }
    format!("{how_it_ended}: {}", tail(&squash(said), STDERR_TAIL))
}

/// The last `STDOUT_LINES_KEPT` non-empty lines of a command lane's stdout,
/// each squashed to one line and cut to `STDERR_TAIL` characters, with a
/// count of what was dropped when there was more than that to keep.
///
/// NON-EMPTY, because a talkative child pads its output with blank lines that
/// would otherwise crowd out the ones that say something.
pub(super) fn stdout_lines(stdout: &str) -> Vec<String> {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let dropped = lines.len().saturating_sub(STDOUT_LINES_KEPT);
    let mut kept: Vec<String> = lines
        .iter()
        .skip(dropped)
        .map(|line| tail(&squash(line), STDERR_TAIL))
        .collect();
    if dropped > 0 {
        kept.insert(0, format!("... {dropped} earlier line(s) dropped"));
    }
    kept
}

/// Text with every control character (embedded CR, stray control bytes)
/// mapped to a space and every backtick mapped to a plain quote, shared by
/// `failure_reason` and a command lane's stdout lines: untrusted child
/// output crosses into the record and out to Discord, where a control
/// character would reflow or truncate a line and three backticks would open
/// or close a code fence around every record line after it.
fn squash(line: &str) -> String {
    line.chars()
        .map(|letter| match letter {
            _ if letter.is_control() => ' ',
            '`' => '\'',
            _ => letter,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- the shared tail -------------------------------------------------------

    #[test]
    fn tail_returns_the_text_unchanged_when_it_already_fits() {
        assert_eq!(tail("hello", 10), "hello");
    }

    #[test]
    fn tail_keeps_the_last_keep_characters_and_prefixes_the_cut() {
        assert_eq!(tail("0123456789ABCDEF", 4), "...CDEF");
    }

    #[test]
    fn tail_cuts_on_a_character_boundary_not_a_byte_offset() {
        // Each "party popper" is 4 bytes. A byte-offset cut (`text.len() -
        // keep`) lands inside the third one's encoding and panics; only a
        // char-aware cut keeps the last 4 CHARACTERS, "\u{1F389}ABC".
        assert_eq!(tail("\u{1F389}\u{1F389}\u{1F389}ABC", 4), "...\u{1F389}ABC");
    }

    #[test]
    fn tail_with_nothing_to_keep_is_only_the_cut_mark() {
        // Asking for zero characters must not hand back the whole text.
        assert_eq!(tail("abc", 0), "...");
    }

    // --- why a command failed -------------------------------------------------

    #[test]
    fn a_failure_reason_carries_what_the_command_printed_on_stderr() {
        assert_eq!(
            failure_reason("exit 1", "fatal: repository not found\n"),
            "exit 1: fatal: repository not found"
        );
    }

    #[test]
    fn a_command_that_said_nothing_reports_only_how_it_ended() {
        // An empty stderr must not leave a dangling colon with nothing after
        // it, which reads as a message that went missing.
        for silence in ["", "\n", "   \n\t"] {
            assert_eq!(failure_reason("exit 2", silence), "exit 2");
        }
        assert_eq!(
            failure_reason("killed by a signal", ""),
            "killed by a signal"
        );
    }

    #[test]
    fn a_talkative_command_is_cut_to_the_tail_that_holds_its_verdict() {
        // A build log's worth of stderr would push every other line off the
        // record and blow past the one card an alert gets, and the verdict is
        // at the END of it.
        let noise = format!("{}THE REAL REASON", "x".repeat(4000));
        let reason = failure_reason("exit 1", &noise);
        assert!(reason.ends_with("THE REAL REASON"), "{reason}");
        // Exact, not a loose upper bound: a mutant that kept only half of
        // STDERR_TAIL would still satisfy "<= STDERR_TAIL + 32" and quietly
        // drop half the promised diagnostic.
        assert_eq!(
            reason.chars().count(),
            "exit 1: ...".chars().count() + STDERR_TAIL,
            "{} characters",
            reason.chars().count()
        );
        assert!(reason.contains("..."), "the cut is visible: {reason}");
    }

    #[test]
    fn a_talkative_command_with_multibyte_stderr_is_cut_on_a_character_boundary() {
        // The same cut, but through stderr that is not one byte per
        // character: a byte-slicing mutant would panic or split a code point
        // instead of keeping whole characters, same failure mode as `tail`
        // itself.
        let noise = format!(
            "{}\u{65e5}\u{672c}\u{8a9e}\u{306e}REASON",
            "\u{3042}".repeat(4000)
        );
        let reason = failure_reason("exit 1", &noise);
        assert!(
            reason.ends_with("\u{65e5}\u{672c}\u{8a9e}\u{306e}REASON"),
            "{reason}"
        );
        assert_eq!(
            reason.chars().count(),
            "exit 1: ...".chars().count() + STDERR_TAIL,
            "{} characters",
            reason.chars().count()
        );
    }

    #[test]
    fn a_multi_line_stderr_is_squashed_onto_one_line() {
        // Lane lines are indented under their lane in the record and go out as
        // one alert sentence; an embedded newline breaks both.
        let reason = failure_reason("exit 1", "first\nsecond\r\nthird");
        assert!(!reason.contains('\n'), "{reason}");
        assert!(!reason.contains('\r'), "{reason}");
        assert!(reason.contains("third"), "{reason}");
    }
    // --- stdout_lines -----------------------------------------------------------

    #[test]
    fn stdout_lines_keeps_everything_when_there_is_little_to_drop() {
        assert_eq!(
            stdout_lines("a\nb\n"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn stdout_lines_drops_blank_lines_rather_than_counting_them_as_content() {
        assert_eq!(
            stdout_lines("a\n\n\nb\n"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn stdout_lines_squashes_a_control_character_embedded_in_one_line() {
        assert_eq!(stdout_lines("a\rb\n"), vec!["a b".to_string()]);
    }

    #[test]
    fn stdout_lines_cuts_an_overlong_multibyte_line_to_its_exact_tail() {
        // Every OTHER stdout_lines test here is short enough that dropping the
        // `tail(..., STDERR_TAIL)` cut still passes; a mutant like that needs
        // a line over the 240-character cap, and multibyte so a byte-indexed
        // cut would panic or land mid-character instead of matching this.
        let filler = "é".repeat(300);
        let line = format!("{filler}TAIL-MARKER");
        let expected = format!("...{}TAIL-MARKER", "é".repeat(229));
        assert_eq!(stdout_lines(&format!("{line}\n")), vec![expected]);
    }

    #[test]
    fn squash_replaces_backticks_so_no_child_line_can_open_or_close_a_code_fence() {
        let squashed = squash("before ``` after");
        assert!(!squashed.contains('`'), "{squashed:?}");
    }

    #[test]
    fn stdout_lines_says_so_when_exactly_one_line_was_dropped() {
        // The boundary: one over the cap drops one line, and that one is
        // still announced.
        let text: String = (1..=STDOUT_LINES_KEPT + 1)
            .map(|number| format!("line {number}\n"))
            .collect();
        let kept = stdout_lines(&text);
        assert_eq!(kept.len(), STDOUT_LINES_KEPT + 1, "{kept:?}");
        assert_eq!(kept[0], "... 1 earlier line(s) dropped");
        assert_eq!(kept[1], "line 2");
    }
}
