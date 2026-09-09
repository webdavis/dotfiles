//! The single chokepoint every rendered value passes through.
//!
//! SHARED BY THE PAGE AND THE DIGEST, which is why it sits here rather than
//! inside either of them. Both render the same attacker-influenceable fields
//! into the same markdown, so a second copy would be a second place for the
//! escaping to be got wrong, and only one of them would be found when it was.
//!
//! THE PAGE FANS OUT TO DISCORD, so every value here is attacker-influenceable
//! text (a launchd label, a path, a certificate subject) rendered into markdown.
//! Two characters do the damage: a backtick ends the inline-code span, and a
//! newline breaks out of it onto a line of its own, where an attacker can forge
//! a line the operator reads as ours. Both are removed here rather than at each
//! call site, so there is one place to check rather than fifteen.

/// The longest a code-wrapped field may be before it is cut.
///
/// One giant value must not spend the whole delivery budget on its own. The
/// whole-page cap is the backstop; this is what keeps a single field from
/// reaching it.
pub(crate) const FIELD_LIMIT: usize = 240;

/// What marks a field cut short.
pub(crate) const FIELD_TRUNCATION: &str = "…(truncated)";

/// Whitespace that would move rendered text onto a line of its own.
const LINE_BREAKING: [char; 3] = ['\r', '\n', '\t'];

/// A value rendered inside a Discord inline-code span.
///
/// Backticks are stripped, line-breaking whitespace becomes spaces, and the
/// result is cut to `FIELD_LIMIT` CHARACTERS rather than bytes, which is what
/// the jq this replaces counted: cutting bytes would split a multi-byte
/// character and render a replacement glyph in its place.
pub(crate) fn code(value: &str) -> String {
    let squashed: String = value
        .chars()
        .filter(|character| *character != '`')
        .map(|character| {
            if LINE_BREAKING.contains(&character) {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut wrapped = String::with_capacity(squashed.len() + 2);
    wrapped.push('`');
    if squashed.chars().count() > FIELD_LIMIT {
        wrapped.extend(squashed.chars().take(FIELD_LIMIT));
        wrapped.push_str(FIELD_TRUNCATION);
    } else {
        wrapped.push_str(&squashed);
    }
    wrapped.push('`');
    wrapped
}

/// Line-breaking whitespace squashed to spaces, and nothing else.
///
/// THE FILE-EVENT ACTION'S EXCEPTION. It is neither code-wrapped nor cut,
/// because the action is a short osquery verb rather than a value an attacker
/// chooses, and rendering it in code would make the line read as a path. It
/// still cannot escape onto its own line, which is the property that matters.
/// The whole-page cap remains its only length bound.
pub(crate) fn squash(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if LINE_BREAKING.contains(&character) {
                ' '
            } else {
                character
            }
        })
        .collect()
}

/// A signing verdict, ready to render.
///
/// THE OTHER EXCEPTION, and it strips MORE than `code` does rather than less:
/// an asterisk goes too. The verdict renders outside a code span so the warning
/// can be bolded, which is exactly where a crafted certificate subject carrying
/// `**` would otherwise inject emphasis of its own. It takes no field cap,
/// because a verdict the operator cannot read in full is worse than a long one.
pub(crate) fn signing(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '`' && *character != '*')
        .map(|character| {
            if LINE_BREAKING.contains(&character) {
                ' '
            } else {
                character
            }
        })
        .collect()
}

/// A path as a single shell word.
///
/// EVERY NEXT STEP IS A LINE THE OPERATOR PASTES INTO A TERMINAL, so a path
/// carrying a quote or a command substitution must arrive as data. Single
/// quotes take the whole word literally, and the one character they cannot
/// carry, a single quote, is closed, escaped and reopened.
pub(crate) fn shell_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for character in value.chars() {
        if character == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('\'');
    quoted
}

/// The last path segment, which is what a secret file renders instead of itself.
pub(crate) fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
