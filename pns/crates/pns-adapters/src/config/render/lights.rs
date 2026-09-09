use super::*;

/// The `[lights]` cluster: one presence flag governs seven headings, because
/// `Config.lights` is one `Option` for the whole table, never seven.
///
/// EVERY CLUSTER AND DECLARATION MAP IS PULLED OUT OF `lights` FIRST, before
/// any of it is written: `render_block`'s own leftover check would otherwise
/// see the whole cluster sitting unclaimed under the bare `[lights]` heading,
/// which serves only `refresh_secs`, and refuse it as an unknown key before
/// the walk ever reaches `[lights.done]`.
pub(super) fn render_lights(out: &mut String, remaining: &mut toml::Table) -> Result<(), String> {
    let present = remaining.contains_key("lights");
    let mut lights = take_table(remaining, "lights")?;

    let mut own_keys = toml::Table::new();
    for key in ["note", "refresh_secs"] {
        if let Some(value) = lights.remove(key) {
            own_keys.insert(key.to_string(), value);
        }
    }
    let mut clusters = Vec::new();
    for cluster in ["done", "failed", "blocked", "unread", "loop", "dim"] {
        clusters.push((cluster, take_table(&mut lights, cluster)?));
    }
    let mut declarations = Vec::new();
    for level in ["lamp", "room", "zone"] {
        declarations.push((level, take_table(&mut lights, level)?));
    }
    if let Some(name) = lights.keys().next() {
        return Err(format!("unknown `lights` key `{name}`"));
    }

    render_block(out, find_table("lights"), &mut own_keys, present)?;
    for (cluster, mut settings) in clusters {
        render_block(
            out,
            find_table(&format!("lights.{cluster}")),
            &mut settings,
            present,
        )?;
    }
    out.push_str(ROUTING);
    if declarations.iter().all(|(_, names)| names.is_empty()) {
        out.push_str(EXAMPLE_DECLARATION);
    }
    for (level, names) in declarations {
        for (name, entry) in names {
            let toml::Value::Table(mut settings) = entry else {
                return Err(format!("`lights.{level}.{name}` is not a table"));
            };
            render_target(out, level, &name, &mut settings)?;
        }
    }
    Ok(())
}

/// One `[lights.lamp."<name>"]`, `[lights.room."<name>"]` or
/// `[lights.zone."<name>"]` declaration.
pub(super) fn render_target(
    out: &mut String,
    level: &str,
    name: &str,
    settings: &mut toml::Table,
) -> Result<(), String> {
    write_note(out, take_note(settings)?);
    out.push_str(&format!("[lights.{level}.{}]\n", quoted(name)));
    for key in ["shows", "dim_window", "dim_behaviours"] {
        if let Some(value) = settings.remove(key) {
            let rendered = render_value(&value)
                .map_err(|error| format!("`lights.{level}.{name}` key `{key}`: {error}"))?;
            out.push_str(&format!("{key} = {rendered}\n"));
        }
    }
    out.push('\n');
    if let Some(name) = settings.keys().next() {
        return Err(format!("unknown `lights.{level}` key `{name}`"));
    }
    Ok(())
}
