//! Key notation: the shell-agnostic spelling a person writes in the table,
//! and its translation into readline's escapes.
//!
//! A chord is space-separated tokens. Each is `ctrl-<char>`, `alt-<char>`
//! (readline's meta), one of the named keys below, or one literal character.

/// A token the notation does not define.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownToken(pub String);

/// Translate a chord such as `ctrl-g a a` into readline's `\C-gaa`.
///
/// The result is meant for the inside of a readline double-quoted key
/// string, so a literal `"` or `\` is escaped for that context.
pub fn to_readline(chord: &str) -> Result<String, UnknownToken> {
    let mut out = String::new();
    for token in chord.split_whitespace() {
        out.push_str(&token_to_readline(token)?);
    }
    Ok(out)
}

fn token_to_readline(token: &str) -> Result<String, UnknownToken> {
    if let Some(rest) = token.strip_prefix("ctrl-") {
        return modified(rest, 'C', token);
    }
    if let Some(rest) = token.strip_prefix("alt-") {
        return modified(rest, 'M', token);
    }
    let named = match token {
        "esc" => Some("\\e"),
        "enter" => Some("\\r"),
        "tab" => Some("\\t"),
        "space" => Some(" "),
        "backspace" => Some("\\b"),
        _ => None,
    };
    if let Some(escape) = named {
        return Ok(escape.to_string());
    }
    let mut characters = token.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Ok(escape_literal(character)),
        _ => Err(UnknownToken(token.to_string())),
    }
}

/// `ctrl-<char>` and `alt-<char>` each carry exactly one character.
fn modified(rest: &str, marker: char, token: &str) -> Result<String, UnknownToken> {
    let mut characters = rest.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Ok(format!("\\{marker}-{character}")),
        _ => Err(UnknownToken(token.to_string())),
    }
}

/// Inside readline's double quotes only these two characters need a backslash.
fn escape_literal(character: char) -> String {
    match character {
        '"' | '\\' => format!("\\{character}"),
        _ => character.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_token_becomes_a_control_escape() {
        assert_eq!(to_readline("ctrl-a").unwrap(), "\\C-a");
    }

    #[test]
    fn alt_token_becomes_a_meta_escape() {
        assert_eq!(to_readline("alt-e").unwrap(), "\\M-e");
    }

    #[test]
    fn named_tokens_become_their_escapes() {
        assert_eq!(to_readline("esc").unwrap(), "\\e");
        assert_eq!(to_readline("enter").unwrap(), "\\r");
        assert_eq!(to_readline("tab").unwrap(), "\\t");
        assert_eq!(to_readline("space").unwrap(), " ");
        assert_eq!(to_readline("backspace").unwrap(), "\\b");
    }

    #[test]
    fn a_bare_character_is_itself() {
        assert_eq!(to_readline("Z").unwrap(), "Z");
        assert_eq!(to_readline(".").unwrap(), ".");
    }

    #[test]
    fn a_quote_or_backslash_is_escaped_for_readline() {
        assert_eq!(to_readline("\"").unwrap(), "\\\"");
        assert_eq!(to_readline("\\").unwrap(), "\\\\");
    }

    #[test]
    fn a_multi_token_chord_concatenates_its_tokens() {
        assert_eq!(to_readline("ctrl-g a a").unwrap(), "\\C-gaa");
        assert_eq!(to_readline("esc [ Z").unwrap(), "\\e[Z");
        assert_eq!(to_readline("ctrl-x ctrl-u").unwrap(), "\\C-x\\C-u");
    }

    #[test]
    fn an_unknown_token_is_refused() {
        assert_eq!(
            to_readline("hyper-a"),
            Err(UnknownToken("hyper-a".to_string()))
        );
        assert_eq!(to_readline("ctrl-ab"), Err(UnknownToken("ctrl-ab".into())));
    }
}
