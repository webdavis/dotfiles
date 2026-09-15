use pns_domain::routes::Routes;
use std::collections::BTreeMap;

/// Every hermes route the config named, with the signing key it granted, out
/// of the `[plugins.hermes.keys]` table.
///
/// THIS TABLE IS THE ROSTER (operator ruling, 2026-09-15). pns compiles in no
/// list of routes: a route the operator granted a key to is a route they
/// granted, whatever it is called and whichever tool posts to it, and a route
/// with no key is one pns never heard of. Deriving the set from the grant is
/// what lets the same binary serve a gateway whose routes this repository has
/// never seen.
///
/// NO `Debug`, and it stays that way, for `router_api_key`'s reason: the keys
/// never enter a type that derives one, so they cannot ride a formatted dump
/// into a log line.
///
/// A ROUTE WITH NO USABLE KEY HAS NO KEY, and there is deliberately no
/// fallback to another route's: the signature is what authorizes a delivery,
/// so signing a route with a key nobody granted it is the hole per-route keys
/// exist to close. The post is refused instead, out loud, naming the route.
#[derive(Default, Clone)]
pub struct HermesKeys(BTreeMap<String, String>);

impl HermesKeys {
    /// The signing key for one route, or None for a route the config granted
    /// no usable key to.
    pub fn key_for(&self, route: &str) -> Option<&str> {
        self.0
            .get(route)
            .map(String::as_str)
            .filter(|key| !key.is_empty())
    }

    /// Every route that would refuse a post, in name order.
    ///
    /// THE TWO ROUTES pns SELECTS ARE ALWAYS ASKED ABOUT, so a machine whose
    /// table is absent altogether still hears that its own events have
    /// nowhere to go. The rest are the routes the table NAMED and left
    /// unarmed: a key line written empty is that route switched off, which is
    /// worth a line, while a route nothing has ever named is not pns's to
    /// invent.
    pub fn unarmed<'a>(&'a self, routes: &'a Routes) -> Vec<&'a str> {
        let mut names: Vec<&str> = self
            .0
            .keys()
            .map(String::as_str)
            .chain([routes.default_route(), routes.urgent_route()])
            .filter(|route| self.key_for(route).is_none())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// The routes and keys out of the `[plugins.hermes]` settings: the `keys`
/// table, route by route. Silent, like every not-set-up reading.
///
/// EVERY WAY A ROUTE CAN FAIL TO STATE A KEY READS AS NOT SET UP for that
/// route alone (no table, a `keys` that is not a table, a non-string value, an
/// empty string), and the other routes keep the keys they did state. A route
/// whose key line is unusable is still NAMED here, because the operator wrote
/// it and the doctor has a line to print about it.
pub fn hermes_keys(settings: &toml::Table) -> HermesKeys {
    let Some(table) = settings.get("keys").and_then(toml::Value::as_table) else {
        return HermesKeys::default();
    };
    HermesKeys(
        table
            .iter()
            .map(|(route, value)| {
                (
                    route.clone(),
                    value.as_str().unwrap_or_default().to_string(),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_way_the_settings_can_fail_to_provide_a_key_reads_not_set_up() {
        for settings in [
            "",
            "other = \"x\"\n",
            "keys = \"secret\"\n",
            "[keys]\n",
            "[keys]\npns-events = \"\"\n",
            "[keys]\npns-events = 42\n",
        ] {
            assert_eq!(
                hermes_keys(&settings.parse().unwrap()).key_for("pns-events"),
                None,
                "case: {settings:?}"
            );
        }
    }

    #[test]
    fn each_route_answers_with_its_own_key_and_nobody_elses() {
        let keys = hermes_keys(
            &"[keys]\nlogbook = \"for-logbook\"\nsirens = \"for-sirens\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(keys.key_for("logbook"), Some("for-logbook"));
        assert_eq!(keys.key_for("sirens"), Some("for-sirens"));
        // THE POINT OF THE WHOLE TABLE: a route the file named no key for
        // does not inherit one, however many other routes are set up.
        assert_eq!(keys.key_for("posture-pages"), None);
    }

    #[test]
    fn a_route_name_this_crate_has_never_heard_of_is_granted_its_key() {
        // THE MUTANT THIS PINS: the roster restored. A key table is the only
        // statement of which routes exist, so a name nothing in pns mentions
        // has to work exactly like one it ships a default for.
        let keys = hermes_keys(
            &"[keys]\nweather-balloons = \"for-balloons\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(keys.key_for("weather-balloons"), Some("for-balloons"));
    }

    #[test]
    fn the_routes_with_no_key_are_the_named_ones_plus_the_two_pns_selects() {
        let routes = Routes::named("logbook", "sirens");
        let keys = hermes_keys(
            &"[keys]\nlogbook = \"armed\"\nweather-balloons = \"\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(keys.unarmed(&routes), ["sirens", "weather-balloons"]);
        assert_eq!(
            HermesKeys::default().unarmed(&routes),
            ["logbook", "sirens"],
            "a machine with no table at all still hears about its own two routes"
        );
    }
}
