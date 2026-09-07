use super::*;

/// `repos`, the repositories the merged pull requests are read from: a list of
/// names in `gh`'s own `OWNER/REPO` spelling, passed to it as one argument
/// each.
///
/// EMPTINESS IS REFUSED AT BOTH LEVELS, for `summarizer`'s reason rather than a
/// new one. A key present with no name under it, or a name that is the empty
/// string, would leave the section reading "nothing merged in this window" over
/// a night that merged plenty, and the operator would be looking at their
/// repository rather than at their config.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT, and deliberately. `gh` accepts
/// `OWNER/REPO`, `HOST/OWNER/REPO` and a full URL, it is the authority on which
/// of those exist, and a shape rule written here would refuse a spelling that
/// works. It is passed as ARGV, so nothing in it can be read as syntax by
/// anything; a name `gh` does not know costs the section one "unavailable"
/// line, which is the same rung a missing `gh` takes.
pub(super) fn repositories(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("recap", "repos", "a list of repository names", setting)?;
    if names.is_empty() || names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`recap` key `repos` names no repository to read".to_string(),
        ));
    }
    Ok(names)
}

/// `review_notes`, the one pattern deciding which files the recap may open.
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
            "`recap` key `review_notes` has type `{}`, not a path with a file name in it",
            setting.type_str()
        )));
    };
    let (directory, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if name.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `review_notes` names no file to read".to_string(),
        ));
    }
    if !pattern.starts_with('/') && !pattern.starts_with("~/") {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, which is not an absolute path or a `~/` one"
        )));
    }
    if directory.contains('*') {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and only its file name may hold a `*`"
        )));
    }
    // AND EXACTLY ONE OF THEM, because that is all the matcher reads. A second
    // `*` is matched LITERALLY, so a pattern carrying one silently matches
    // nothing at all, which is the outcome every refusal in this file exists to
    // turn into a sentence the operator can act on.
    if name.matches('*').count() > 1 {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and its file name may hold only one `*`"
        )));
    }
    Ok(pattern.to_string())
}
