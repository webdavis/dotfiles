//! The failure messages quote config keys by name. This is the only place that
//! can check them, because the wording lives in `pns-domain` and the schema
//! lives here, and the domain does not depend on the adapters.
//!
//! NAMING A CONFIG KEY INSIDE AN ERROR IS A COMMITMENT. Rename the key and the
//! message goes stale silently: it keeps telling the operator to edit a key that
//! no longer exists, which is worse than saying nothing, because they will go
//! and look for it. These cases turn that into a build failure.

use crate::config::schema::TABLE_KEYS;
use pns_domain::failure::{HERMES_KEY, MOBILE_TOKEN};

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
    for quoted in [HERMES_KEY, MOBILE_TOKEN] {
        assert!(
            declared(quoted),
            "{quoted} is quoted in a failure message but the schema declares no such key"
        );
    }
}

/// The guard above is only worth having if it can fail, and the shape it parses
/// is easy to get subtly wrong. This pins that a key the schema does not declare
/// is rejected, rather than the parse quietly returning true for everything.
#[test]
fn a_quoted_key_the_schema_does_not_declare_is_rejected() {
    assert!(!declared("[plugins.hermes] keys"));
    assert!(!declared("[plugins.hermes.nested] key"));
    assert!(!declared("[plugins.hermes]key"));
    assert!(!declared("plugins.hermes key"));
}
