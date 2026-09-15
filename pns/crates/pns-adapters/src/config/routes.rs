use super::*;

/// `[routes]`: what the two routes pns selects for itself are called.
///
/// EITHER KEY FALLS BACK TO ITS DEFAULT, so a file with no table, or one
/// naming only the urgent route, is a complete statement. An unusable name is
/// REFUSED BY NAME rather than swapped for the default: a route that cannot
/// stand as a URL path segment is a page that would silently land somewhere
/// else, and the operator is two words from the key they have to fix.
pub(super) fn parse_routes(value: toml::Value) -> Result<pns_domain::routes::Routes, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`routes` is not a table".into()));
    };
    let shipped = pns_domain::routes::Routes::default();
    for key in table.keys() {
        admits_flat(schema::ROUTES, key)?;
    }
    Ok(pns_domain::routes::Routes::named(
        &name(&table, "default", shipped.default_route())?,
        &name(&table, "urgent", shipped.urgent_route())?,
    ))
}

/// One route name off the table, or `fallback` when the table leaves it out.
fn name(table: &toml::Table, key: &str, fallback: &str) -> Result<String, ConfigError> {
    let Some(value) = table.get(key) else {
        return Ok(fallback.to_string());
    };
    let Some(route) = value.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`routes` key `{key}` has type `{}`, not a string",
            value.type_str()
        )));
    };
    if !pns_domain::safety::route_name_is_usable(route) {
        return Err(ConfigError::Invalid(format!(
            "`routes` key `{key}` is not a usable route name; \
             a route is letters, digits, `-` and `_`"
        )));
    }
    Ok(route.to_string())
}

#[cfg(test)]
#[path = "routes/tests.rs"]
mod tests;
