use super::*;

/// `min_events`, the volume threshold. A negative or fractional value is
/// refused BY NAME rather than clamped: the operator asked for a threshold, and
/// a silently corrected one is a threshold they believe they set.
pub(super) fn threshold(setting: &toml::Value) -> Result<usize, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `min_events` has type `{}`, not a count",
            setting.type_str()
        )));
    };
    // AND ZERO IS REFUSED BY NAME TOO, for the same reason a negative is.
    // `counted.len() >= 0` is always true, so a zero threshold recaps EVERY
    // event, including one over an empty window, which is the state
    // `recap::NOTHING_HAPPENED` says the event path never posts. An operator
    // calibrating the knob downward gets a card and a Discord recap on every
    // event, each saying nothing was recorded. One is the floor: it means "any
    // activity at all".
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`recap` key `min_events` is 0, which is not a threshold; 1 is the floor".to_string(),
        ));
    }
    Ok(count)
}

/// `summarizer`, the command the window is handed to: a list of WORDS, passed
/// to the process directly and never through a shell.
///
/// THE SHELL STRING IS THE MISTAKE THIS REFUSES. `summarizer = "ollama run
/// qwen3.5:4b"` is what a hand writes first, and reading it as a one-word
/// command would name a binary nobody has, so it is refused by name rather than
/// left to fail once a night inside a detached process.
///
/// AN EMPTY LIST IS REFUSED TOO, because it names no command at all: taken as
/// written it would leave the summarizer configured and unrunnable, which reads
/// to the operator as a summarizer that is not answering rather than as a table
/// they have to fix.
///
/// AND SO IS AN EMPTY FIRST WORD, on that same reasoning rather than a new one.
/// `[""]` parses, `Command::new("")` fails to spawn, and the operator gets
/// precisely the outcome the paragraph above exists to prevent. Only the first
/// word is judged: an empty ARGUMENT is a real thing to pass a program, and
/// nothing about it stops the command running.
pub(super) fn argv(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let words = strings("recap", "summarizer", "a list of command words", setting)?;
    if words.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` is empty, so it names no command to run".to_string(),
        ));
    }
    if words[0].is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` starts with an empty word, so it names no command to run"
                .to_string(),
        ));
    }
    Ok(words)
}
