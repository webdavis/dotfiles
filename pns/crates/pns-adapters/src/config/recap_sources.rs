use super::*;

/// `[recap.sources]`, one argv list per list section.
///
/// A COMMAND, NEVER A REPOSITORY LIST. pns owns no task tool, no apply log and
/// no opinion about which repositories matter, so each section is filled by a
/// program the operator names word by word. `{since}` and `{until}` in any
/// word are replaced with the window's bounds before it runs.
///
/// UNSET IS THE OFF STATEMENT, so no key doubles as its own switch: a section
/// nobody named starts no process, prints nothing and is not a name
/// `--section` accepts.
///
/// EMPTINESS IS REFUSED AT BOTH LEVELS, for `summarizer`'s reason. A key
/// present with an empty list names no command at all, and a first word that
/// is the empty string parses and then fails to spawn, which reads to the
/// operator as a section that is not answering rather than as a table they
/// have to fix. Only the FIRST word is judged: an empty argument is a real
/// thing to pass a program.
///
/// THE WORDS THEMSELVES ARE NOT JUDGED BEYOND THAT, and deliberately. They are
/// passed as argv, so nothing in them can be read as syntax by anything, and a
/// shape rule written here would refuse a spelling that works.
pub(super) fn parse_recap_sources(value: toml::Value) -> Result<Sources, ConfigError> {
    const TABLE: &str = "recap.sources";
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`recap.sources` is not a table".to_string(),
        ));
    };
    let mut sources = Sources::default();
    for (key, setting) in table {
        admits_flat(TABLE, &key)?;
        let command = source_command(&key, &setting)?;
        match key.as_str() {
            "pull_requests" => sources.pull_requests = Some(command),
            "commits" => sources.commits = Some(command),
            "tasks" => sources.tasks = Some(command),
            "applies" => sources.applies = Some(command),
            _ => return Err(unknown_key(TABLE, TABLE, &key)),
        }
    }
    Ok(sources)
}

/// One source command's words, refused by name when they name no program.
fn source_command(key: &str, setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    const TABLE: &str = "recap.sources";
    let words = strings(TABLE, key, "a list of command words", setting)?;
    if words.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` key `{key}` is empty, so it names no command to run"
        )));
    }
    if words[0].is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` key `{key}` starts with an empty word, so it names no command to run"
        )));
    }
    Ok(words)
}

/// `review_notes_glob`, the one pattern deciding which files the recap may open.
///
/// UNSET IS THE OFF STATEMENT, as it is for `repositories` above: with no
/// pattern the directory is never opened.
///
/// THE GLOB IS THE WHOLE PERMISSION, which is why its shape is judged here
/// rather than resolved generously at the read. Two spellings are refused by
/// name and each would widen what pns opens beyond what the operator wrote:
///
/// A RELATIVE PATH resolves against the working directory, and the recap is
/// rendered by a process started from whatever directory the return event fired
/// in. The same key would then name a different set of files on every run, so
/// only an absolute path and a `~/` one are admitted.
///
/// AND A `*` IN A DIRECTORY makes the set of DIRECTORIES a search rather than a
/// statement: `~/.claude/*/checklist-*.md` asks pns to walk directories nobody
/// listed. Only the file name may hold one, which keeps the read to a single
/// directory the operator named in full.
pub(super) fn note_glob(setting: &toml::Value) -> Result<String, ConfigError> {
    let Some(pattern) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes_glob` has type `{}`, not a path with a file name in it",
            setting.type_str()
        )));
    };
    let (directory, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if name.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `review_notes_glob` names no file to read".to_string(),
        ));
    }
    if !pattern.starts_with('/') && !pattern.starts_with("~/") {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes_glob` is `{pattern}`, which is not an absolute path or a `~/` one"
        )));
    }
    if directory.contains('*') {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes_glob` is `{pattern}`, and only its file name may hold a `*`"
        )));
    }
    // AND EXACTLY ONE OF THEM, because that is all the matcher reads. A second
    // `*` is matched LITERALLY, so a pattern carrying one silently matches
    // nothing at all, which is the outcome every refusal in this file exists to
    // turn into a sentence the operator can act on.
    if name.matches('*').count() > 1 {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes_glob` is `{pattern}`, and its file name may hold only one `*`"
        )));
    }
    Ok(pattern.to_string())
}
