use super::*;

/// Every `[profiles.*]` heading, in file order: the named profiles in sorted
/// order, then the locations, then the rules as an ARRAY OF TABLES.
///
/// A HARDCODED BRANCH, like `render_delivery_classes`: the profile names are
/// the operator's own, and `[[profiles.rules]]` is the layout's first array of
/// tables, which a `Table` row cannot describe.
pub(super) fn render_profiles(out: &mut String, remaining: &mut toml::Table) -> Result<(), String> {
    let mut table = take_table(remaining, "profiles")?;
    let locations = take_table(&mut table, "locations")?;
    let rules = match table.remove("rules") {
        None => Vec::new(),
        Some(toml::Value::Array(rows)) => rows,
        Some(other) => {
            return Err(format!(
                "`profiles.rules` has type `{}`, not a list of rules",
                other.type_str()
            ));
        }
    };
    let poll = table.remove("location_poll");
    render_block(out, &PROFILES, &mut named_only(poll), true)?;
    for (name, entry) in table {
        let toml::Value::Table(mut settings) = entry else {
            return Err(format!("`profiles.{name}` is not a table"));
        };
        write_note(out, take_note(&mut settings)?);
        out.push_str(&format!("[profiles.{name}]\n"));
        for key in PROFILE.keys {
            let literal = match settings.remove(key.name) {
                Some(value) => render_value(&value)
                    .map_err(|error| format!("`profiles.{name}` key `{}`: {error}", key.name))?,
                None => match key.sample {
                    Sample::Default(literal) | Sample::Example(literal) => literal.to_string(),
                },
            };
            out.push_str(&format!("{} = {literal}\n", key.name));
        }
        out.push('\n');
        if let Some(key) = settings.keys().next() {
            return Err(format!("unknown `profiles.{name}` key `{key}`"));
        }
    }
    if !locations.is_empty() {
        out.push_str(LOCATIONS_PROSE);
        out.push_str("[profiles.locations]\n");
        for (name, fingerprint) in locations {
            let literal = render_value(&fingerprint)
                .map_err(|error| format!("`profiles.locations` key `{name}`: {error}"))?;
            out.push_str(&format!("{name} = {literal}\n"));
        }
        out.push('\n');
    }
    if !rules.is_empty() {
        out.push_str(RULES_PROSE);
    }
    for row in rules {
        let toml::Value::Table(settings) = row else {
            return Err("`profiles.rules` holds something that is not a rule".to_string());
        };
        out.push_str("[[profiles.rules]]\n");
        // `profile` FIRST and the inputs after it, in the map's own sorted
        // order, which is what makes a regenerated template byte-stable.
        if let Some(named) = settings.get("profile") {
            out.push_str(&format!("profile = {}\n", render_value(named)?));
        }
        for (key, value) in &settings {
            if key == "profile" {
                continue;
            }
            out.push_str(&format!("{key} = {}\n", render_value(value)?));
        }
        out.push('\n');
    }
    Ok(())
}

/// The `[profiles]` heading's own settings: the one knob, and nothing the
/// named tables below it carry.
fn named_only(poll: Option<toml::Value>) -> toml::Table {
    let mut settings = toml::Table::new();
    if let Some(poll) = poll {
        settings.insert("location_poll".to_string(), poll);
    }
    settings
}
