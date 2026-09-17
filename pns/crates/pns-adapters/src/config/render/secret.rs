/// The keepassxc entry fields that may stand as a secret's `field`. Anything
/// else is refused, because `field` becomes a chezmoi method call verbatim.
pub(super) const SECRET_FIELDS: [&str; 2] = ["Password", "UserName"];

/// A secret marker, `{ keepassxc = "<entry>", field = "Password" | "UserName" }`,
/// as the chezmoi action `{{ (keepassxc "<entry>").<field> | toToml }}`, with
/// NO author quotes: `toToml` supplies its own once chezmoi resolves the
/// value, and those are the quotes that end up in the deployed file.
///
/// A RENDERED SECRET IS NOT TOML UNTIL `toToml` RUNS. Go's `quote` (`%q`)
/// would emit escapes TOML does not define (`\a`, `\v`, `\xNN`) for a secret
/// holding a control byte, breaking the whole deployed file from that line
/// on; `toToml` emits `\uXXXX` for the same bytes and round-trips every one
/// of them. Author quotes around the action would only duplicate what
/// `toToml` already writes, so this render's job is to write the bare
/// action, not to quote it.
///
/// NEITHER STRING IS TRUSTED. `field` is checked against the two chezmoi
/// methods a keepassxc entry actually exposes, and the entry name is refused
/// if it carries a quote, a backslash, `}}` or a control character: the
/// first three would close the Go string or the action early and let the rest
/// of the entry name run as template syntax, and a newline would start a line
/// of its own in the rendered text.
pub(super) fn secret_action(table: &toml::Table) -> Result<String, String> {
    // NAME THE OFFENDER FIRST: `table.len() != 2` alone only counts members,
    // so a third, unrecognised one hides behind the generic pair-count
    // message instead of being called out by name.
    for key in table.keys() {
        if key != "keepassxc" && key != "field" && key != "attribute" {
            return Err(format!(
                "a secret table may only hold `keepassxc` and one of `field` or \
                 `attribute`, not `{key}`"
            ));
        }
    }
    if table.len() != 2 {
        return Err(
            "a table value must be a secret: exactly `keepassxc` and one of `field` or \
             `attribute`"
                .to_string(),
        );
    }
    let entry = match table.get("keepassxc") {
        Some(toml::Value::String(entry)) => entry,
        _ => return Err("a secret's `keepassxc` must name the entry as a string".to_string()),
    };
    if entry.trim().is_empty() {
        // AN EMPTY OR WHITESPACE-ONLY ENTRY IS NOT A NAME, and writing it
        // through defers the failure to an apply-time vault lookup for
        // `keepassxc ""`, which the operator hits far from wherever the
        // values file went wrong.
        return Err("a secret's `keepassxc` entry name cannot be blank".to_string());
    }
    quotable(entry).map_err(|why| format!("the keepassxc entry name `{entry}` {why}"))?;
    // AN ATTRIBUTE IS A DIFFERENT CHEZMOI FUNCTION, not a different field. A
    // custom attribute is not one of the two methods `keepassxc` exposes, so a
    // value stored beside the password rather than in it reaches a template
    // through `keepassxcAttribute` and nowhere else.
    if let Some(attribute) = table.get("attribute") {
        let toml::Value::String(attribute) = attribute else {
            return Err("a secret's `attribute` must name the attribute as a string".to_string());
        };
        if attribute.trim().is_empty() {
            return Err("a secret's `attribute` name cannot be blank".to_string());
        }
        quotable(attribute)
            .map_err(|why| format!("the keepassxc attribute name `{attribute}` {why}"))?;
        let mut action = String::with_capacity(entry.len() + attribute.len() + 40);
        action.push_str("{{ keepassxcAttribute \"");
        action.push_str(entry);
        action.push_str("\" \"");
        action.push_str(attribute);
        action.push_str("\" | toToml }}");
        return Ok(action);
    }
    let field = match table.get("field") {
        Some(toml::Value::String(field)) => field,
        _ => return Err("a secret's `field` must be a string".to_string()),
    };
    if !SECRET_FIELDS.contains(&field.as_str()) {
        return Err(format!(
            "a secret's `field` must be one of {SECRET_FIELDS:?}, not `{field}`"
        ));
    }
    // BUILT WITH `push_str` RATHER THAN `format!`, deliberately: the target
    // text is thick with literal `{`, `}` and `"` characters, and escaping all
    // of them inside a format string is exactly the kind of place a stray
    // brace goes unnoticed.
    let mut action = String::with_capacity(entry.len() + field.len() + 28);
    action.push_str("{{ (keepassxc \"");
    action.push_str(entry);
    action.push_str("\").");
    action.push_str(field);
    action.push_str(" | toToml }}");
    Ok(action)
}

/// Whether a name can stand inside a Go template action at all: a quote, a
/// backslash or a `}}` would close the string or the action early and let the
/// rest of the name run as template syntax, and a control character would start
/// a line of its own in the rendered text.
fn quotable(name: &str) -> Result<(), String> {
    (!name.contains('"')
        && !name.contains('\\')
        && !name.contains("}}")
        && !name.chars().any(char::is_control))
    .then_some(())
    .ok_or_else(|| "cannot stand inside a chezmoi action".to_string())
}
