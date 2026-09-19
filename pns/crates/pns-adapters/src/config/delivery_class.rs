use super::*;

/// What one `[delivery_class.<name>]` table says about every message that
/// carries that class.
///
/// THE CLASSES ARE THE OPERATOR'S, WHOLE. pns compiles in no class words:
/// which classes exist, where each one goes and which of them interrupt a
/// mute are all in the file, so a producer's word means what this machine
/// says it means and a word this machine never defined is refused rather
/// than delivered as a guess.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeliveryClass {
    /// `route`: where a message of this class goes when its producer named no
    /// route of its own. EMPTY IS THE DEFAULT ROUTE, which is the empty route
    /// every path already reads as the default, so a class that only bypasses
    /// the mute writes nothing here.
    pub route: String,
    /// `bypass_mute`: whether it passes the timed mute and the named Focus
    /// modes for the banner and the phone card.
    pub bypass_mute: bool,
}

/// The class a message naming none takes. THE RULE WRITTEN DOWN rather than
/// hidden in code: an operator reading the file sees what an unclassed
/// message does, and deleting the table is how they turn that rule off.
pub const DEFAULT_DELIVERY_CLASS: &str = "default";

/// `[delivery_class]`: one nested table per class, keyed by the operator's own
/// class name.
pub(super) fn parse_delivery_classes(
    value: toml::Value,
) -> Result<BTreeMap<String, DeliveryClass>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`delivery_class` is not a table".into(),
        ));
    };
    let mut classes = BTreeMap::new();
    for (name, entry) in table {
        // THE WIRE'S OWN BOUND ON A NAME, so a class an operator can write is
        // a class a producer can send: a longer or emptier one would be a
        // table nothing could ever name.
        if pns_protocol::Name::new(&name).is_err() {
            return Err(ConfigError::Invalid(format!(
                "`delivery_class` name `{name}` is not usable; \
                 a class name is 1 to 64 characters without controls"
            )));
        }
        let shown = format!("delivery_class.{name}");
        let toml::Value::Table(settings) = entry else {
            return Err(ConfigError::Invalid(format!("`{shown}` is not a table")));
        };
        let mut class = DeliveryClass::default();
        for (key, setting) in settings {
            admits(schema::DELIVERY_CLASS_KEYS, &shown, &key)?;
            match key.as_str() {
                "route" => class.route = route_name(&shown, &setting)?,
                "bypass_mute" => {
                    class.bypass_mute = setting.as_bool().ok_or_else(|| {
                        ConfigError::Invalid(format!(
                            "`{shown}` key `bypass_mute` has type `{}`, not boolean",
                            setting.type_str()
                        ))
                    })?;
                }
                _ => return Err(unknown_key(schema::DELIVERY_CLASS_KEYS, &shown, &key)),
            }
        }
        classes.insert(name, class);
    }
    Ok(classes)
}

/// The class's route, on `[routes]`'s own terms: a name that could not stand
/// as a URL path segment is refused rather than swapped for another. THE
/// EMPTY STRING IS ADMITTED, because that is how a class says it takes the
/// default route.
fn route_name(shown: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    let Some(route) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`{shown}` key `route` has type `{}`, not a string",
            setting.type_str()
        )));
    };
    if !route.is_empty() && !pns_domain::safety::route_name_is_usable(route) {
        return Err(ConfigError::Invalid(format!(
            "`{shown}` key `route` is not a usable route name; \
             a route is letters, digits, `-` and `_`"
        )));
    }
    Ok(route.to_string())
}
