use super::*;

/// Every `[delivery_class.<name>]` the caller declared, in the map's own
/// sorted order, which is what makes a regenerated template byte-stable.
///
/// A HARDCODED BRANCH, like the lamp declarations: the headings carry the
/// operator's own class names, so there is no `Key` list for the walk to read
/// and the layout entry states the two keys each declaration serves.
pub(super) fn render_delivery_classes(
    out: &mut String,
    table: &Table,
    remaining: &mut toml::Table,
) -> Result<(), String> {
    let classes = take_table(remaining, last_segment_of_prefix(table.name))?;
    out.push_str(table.prose);
    if classes.is_empty() {
        out.push_str(EXAMPLE_CLASS);
        return Ok(());
    }
    for (name, entry) in classes {
        let toml::Value::Table(mut settings) = entry else {
            return Err(format!("`delivery_class.{name}` is not a table"));
        };
        write_note(out, take_note(&mut settings)?);
        out.push_str(&format!("[delivery_class.{name}]\n"));
        for key in table.keys {
            let literal = match settings.remove(key.name) {
                Some(value) => render_value(&value).map_err(|error| {
                    format!("`delivery_class.{name}` key `{}`: {error}", key.name)
                })?,
                None => match key.sample {
                    Sample::Default(literal) | Sample::Example(literal) => literal.to_string(),
                },
            };
            out.push_str(&format!("{} = {literal}\n", key.name));
        }
        out.push('\n');
        if let Some(key) = settings.keys().next() {
            return Err(format!("unknown `delivery_class.{name}` key `{key}`"));
        }
    }
    Ok(())
}

/// The table name a `<name>`-suffixed roster prefix reads from in a values
/// table: `delivery_class.<name>` is written under `delivery_class`.
fn last_segment_of_prefix(prefix: &str) -> &str {
    prefix.split('.').next().unwrap_or(prefix)
}
