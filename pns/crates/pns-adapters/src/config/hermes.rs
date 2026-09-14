use std::collections::BTreeMap;

/// Every hermes signing key the config names, one per route, out of the
/// `[plugins.hermes.keys]` table.
///
/// NO `Debug`, and it stays that way, for `router_api_key`'s reason: the keys
/// never enter a type that derives one, so they cannot ride a formatted dump
/// into a log line.
///
/// A ROUTE WITH NO ENTRY HAS NO KEY, and there is deliberately no fallback to
/// another route's: the signature is what authorizes a delivery, so signing a
/// route with a key nobody granted it is the hole per-route keys exist to
/// close. The post is refused instead, out loud, naming the route.
#[derive(Default, Clone, PartialEq)]
pub struct HermesKeys(BTreeMap<String, String>);

impl HermesKeys {
    /// The signing key for one route, or None for a route the config names no
    /// key for.
    pub fn key_for(&self, route: &str) -> Option<&str> {
        self.0.get(route).map(String::as_str)
    }
}

/// The signing keys out of the `[plugins.hermes]` settings: the `keys` table,
/// route by route. Silent, like every not-set-up reading.
///
/// EVERY WAY A ROUTE CAN FAIL TO STATE A KEY READS AS NOT SET UP for that
/// route alone (no table, a `keys` that is not a table, a non-string value, an
/// empty string), and the other routes keep the keys they did state. The route
/// NAMES are already judged by the schema, which refuses a name no code posts
/// to before this reader ever sees the table.
pub fn hermes_keys(settings: &toml::Table) -> HermesKeys {
    let Some(table) = settings.get("keys").and_then(toml::Value::as_table) else {
        return HermesKeys::default();
    };
    HermesKeys(
        table
            .iter()
            .filter_map(|(route, value)| {
                let key = value.as_str().filter(|key| !key.is_empty())?;
                Some((route.clone(), key.to_string()))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pns_domain::routes::DEFAULT_ROUTE;

    #[test]
    fn every_way_the_settings_can_fail_to_provide_a_key_reads_not_set_up() {
        for settings in [
            "",
            "other = \"x\"\n",
            "keys = \"secret\"\n",
            "[keys]\n",
            "[keys]\npns = \"\"\n",
            "[keys]\npns = 42\n",
        ] {
            assert_eq!(
                hermes_keys(&settings.parse().unwrap()).key_for(DEFAULT_ROUTE),
                None,
                "case: {settings:?}"
            );
        }
    }

    #[test]
    fn each_route_answers_with_its_own_key_and_nobody_elses() {
        let keys = hermes_keys(
            &"[keys]\npns = \"for-pns\"\npriority = \"for-priority\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(keys.key_for("pns"), Some("for-pns"));
        assert_eq!(keys.key_for("priority"), Some("for-priority"));
        // THE POINT OF THE WHOLE TABLE: a route the file named no key for
        // does not inherit one, however many other routes are set up.
        assert_eq!(keys.key_for("posture"), None);
    }
}
