//! Guards on the values that arrive from outside and are then used as
//! something more dangerous than text: a pane id that becomes a shell word, a
//! session id that becomes a filename, a route name that becomes a URL path
//! segment.

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

/// True when a pane id may become a FILENAME as well as a shell word.
///
/// THE PANE ALLOWLIST PLUS ONE REFUSAL, rather than a third allowlist. herdr's
/// own ids carry a colon (`wW:p21`), so `session_id_is_safe` refuses every real
/// one; what the pane predicate permits and a filename cannot is the parent
/// reference, because a pane id becomes a shell WORD there and never a path.
///
/// AND THE WORKING GRAMMAR LOSES ITS PLACE TOO: `working_owner` reads
/// `<name>.new.<pid>` and `<name>.sweep.<pid>` as ITS OWN writers' working
/// files, and a pane id that happens to spell that shape would be swept by
/// the wrong pid, or never released, the moment it named a lease. No real
/// pane id can spell it (herdr's own ids and `wW:p21` do not carry digits
/// after a `.new.` or `.sweep.` run), so this refuses nothing a caller
/// actually has.
pub fn pane_file_is_safe(pane: &str) -> bool {
    pane_is_safe(pane) && !pane.contains("..") && crate::lights::working_owner(pane).is_none()
}

/// True when a harness-supplied session id may be used as a FILENAME. The id
/// arrives inside a hook payload and is interpolated into a path, so a value
/// carrying a separator or a parent reference would write outside its
/// directory. This allowlist is NARROWER than the pane one: nothing here is a
/// multiplexer id, so the colon has nothing to earn its place with.
///
/// THE SAME WORKING-GRAMMAR REFUSAL as the pane predicate, and for the same
/// reason: a session id shaped like one of this crate's own working files
/// would be misread as one by `working_owner`, and no harness-generated
/// session id spells that shape.
pub fn session_id_is_safe(session_id: &str) -> bool {
    !session_id.is_empty()
        && !session_id.contains("..")
        && session_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        && crate::lights::working_owner(session_id).is_none()
}

/// True when a name may become the final path segment of the hermes gateway
/// URL: non-empty, and nothing outside the unreserved run of ASCII letters,
/// digits, `-` and `_`. Nothing traversal-shaped, space-carrying or
/// query-shaped passes.
///
/// THE ONE RULE, because two readers judge names: `channel_url` when it builds
/// the URL, and the config read that resolves a route by name. Two spellings
/// of "usable" would mean a value one waved through and the other refused,
/// which is a route silently swapped for the default.
pub fn route_name_is_usable(route: &str) -> bool {
    !route.is_empty()
        && route
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests;
