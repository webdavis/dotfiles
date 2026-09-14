//! The failure messages quote config keys by name. This is the only place that
//! can check them, because the wording lives in `pns-domain` and `pns-hermes`
//! and the schema lives here, and neither of those depends on the adapters.
//!
//! NAMING A CONFIG KEY INSIDE AN ERROR IS A COMMITMENT. Rename the key and the
//! message goes stale silently: it keeps telling the operator to edit a key that
//! no longer exists, which is worse than saying nothing, because they will go
//! and look for it. These cases turn that into a build failure.

use crate::config::schema::TABLE_KEYS;
use pns_domain::failure::{MOBILE_TOKEN, hermes_key_named};
use pns_domain::routes::ROUTES;

/// A quoted key, split back into the table and key the roster states.
fn declared(quoted: &str) -> bool {
    let Some((table, key)) = quoted
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    else {
        return false;
    };
    TABLE_KEYS
        .iter()
        .any(|(name, keys)| *name == table && keys.contains(&key))
}

#[test]
fn every_config_key_a_failure_message_quotes_is_a_key_the_schema_declares() {
    let mut quoted = vec![MOBILE_TOKEN.to_string()];
    // ONE PER ROUTE, because the hermes wording names the route's own key and
    // a roster missing a route is a message pointing at a key the config
    // refuses.
    quoted.extend(ROUTES.iter().map(|route| hermes_key_named(route)));
    for quoted in quoted {
        assert!(
            declared(&quoted),
            "{quoted} is quoted in a failure message but the schema declares no such key"
        );
    }
}

/// The no-key refusal is composed in `pns-hermes`, which cannot see the schema
/// either, and it is the one an operator meets FIRST: it fires before any post
/// leaves the machine. So its config path is held to the same standard as the
/// gateway's own answers.
#[test]
fn the_no_key_refusal_quotes_a_key_the_schema_declares() {
    for route in ROUTES {
        let said = pns_hermes::skipped_line(route);
        let quoted = hermes_key_named(route);
        assert!(
            said.contains(&quoted),
            "the refusal for {route} must quote {quoted}: {said}"
        );
        assert!(declared(&quoted), "{quoted} is not a key the schema serves");
    }
}

/// The guard above is only worth having if it can fail, and the shape it parses
/// is easy to get subtly wrong. This pins that a key the schema does not declare
/// is rejected, rather than the parse quietly returning true for everything.
#[test]
fn a_quoted_key_the_schema_does_not_declare_is_rejected() {
    assert!(!declared("[plugins.hermes] key"));
    assert!(!declared("[plugins.hermes.nested] pns"));
    assert!(!declared("[plugins.hermes.keys]pns"));
    assert!(!declared("plugins.hermes.keys pns"));
    assert!(!declared("[plugins.hermes.keys] general"));
}
