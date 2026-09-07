/// A chezmoi-templated text with its actions taken out: a directive standing
/// on its own line goes with the line, and an action inside a value becomes
/// `placeholder`.
///
/// THE SHARED STUB, lifted out of this module's own test for the shipped
/// template so `config_text`'s tests can fake-render a secret action the same
/// way: a rendered secret action carries no author quotes of its own (`|
/// toToml` supplies them once chezmoi resolves the value), so `placeholder`
/// must be a quoted string for the substituted text to stand in for what
/// chezmoi would actually have produced. A round-trip test has to stand in
/// for chezmoi before it hands the text to `parse_config`, and one stub is
/// what keeps that standing-in from drifting between the two callers.
///
/// ONLY THAT ONE ACTION IS STOOD IN FOR. An action in value position must
/// read exactly `{{ (keepassxc "<entry>").<field> | toToml }}`, the text
/// `config_text::secret_action` writes; anything else is refused. Swapping a
/// quoted placeholder in for ANY action would let a template line that
/// dropped `| toToml` keep every template test green while chezmoi splices
/// the raw vault bytes in unquoted.
///
/// NOT TEST-ONLY: `pns-config-render` calls this at runtime too, to stand in
/// for chezmoi before self-parsing its own render, so it returns a refusal
/// naming the offender rather than panicking.
///
/// `placeholder` IS HANDED THE MATCHED ACTION'S OWN ENTRY AND FIELD, not a
/// fixed string chosen by the caller: a single fixed placeholder stubs every
/// secret in a multi-secret text to the SAME value, so a table-comparison
/// test built on top of it cannot tell a swapped entry from an unswapped
/// one, sol-1 finding 1 (two tables comparing equal after their secrets
/// traded places). A caller that genuinely wants one fixed value back for
/// every action (the round-trip tests below, each with exactly one secret)
/// just ignores the two arguments.
pub fn strip_chezmoi_actions(
    text: &str,
    placeholder: impl Fn(&str, &str) -> String,
) -> Result<String, String> {
    let mut lines = Vec::new();
    for line in text
        .lines()
        .filter(|line| !line.trim_start().starts_with("{{-"))
    {
        let mut rendered = line.to_string();
        while let Some(start) = rendered.find("{{") {
            let Some(end) = rendered[start..]
                .find("}}")
                .map(|offset| start + offset + 2)
            else {
                return Err(format!(
                    "a chezmoi action is not closed on its own line: {rendered}"
                ));
            };
            let action = &rendered[start..end];
            let secret_identity = action
                .strip_prefix("{{ (keepassxc \"")
                .and_then(|rest| rest.split_once("\")."))
                .filter(|(entry, _)| !entry.contains('"'))
                .and_then(|(entry, rest)| {
                    super::SECRET_FIELDS
                        .iter()
                        .find(|field| rest == format!("{field} | toToml }}}}"))
                        .map(|field| (entry, *field))
                });
            let Some((entry, field)) = secret_identity else {
                return Err(format!("not a `| toToml` secret action: {action}"));
            };
            rendered.replace_range(start..end, &placeholder(entry, field));
        }
        lines.push(rendered);
    }
    Ok(lines.join("\n"))
}

/// A `strip_chezmoi_actions` placeholder that carries the action's own entry
/// and field, so no two DIFFERENT secrets in a multi-secret text can stub to
/// the same value: a comparison built on top of this stub can tell a secret
/// that moved to a different table apart from one that did not, which the
/// single fixed string every caller used before could not (sol-1 finding 1).
/// The backslash escape is defensive: `entry` is already known quote-free by
/// the caller above, but not backslash-free, and this keeps the result a
/// valid TOML basic string either way.
pub fn identity_placeholder(entry: &str, field: &str) -> String {
    format!("\"from-the-vault:{}:{field}\"", entry.replace('\\', "\\\\"))
}
