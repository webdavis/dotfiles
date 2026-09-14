//! The hermes routes pns posts to, named once.
//!
//! ONE ROSTER, because three layers ask the same question and used to answer
//! it with three literals: the config schema decides which route names
//! `[plugins.hermes.keys]` may carry, the signer looks a key up by the route
//! it is about to post to, and the failure wording quotes that key back at the
//! operator. A route added in one of the three and missed in the others is a
//! key the config refuses, or one nothing ever reads.

/// The route an event that named none takes: the gateway's own `pns` webhook,
/// which is also the final segment of `DEFAULT_HERMES_URL`.
pub const DEFAULT_ROUTE: &str = "pns";

/// The route a threaded recap posts to, read by `post_return_recap`.
pub const RECAP_ROUTE: &str = "pns-recap";

/// The route a page submitted by the posture pipeline posts to. NOTHING IN
/// PNS SELECTS IT: posture is the producer that names it, and this is the wire
/// name its own `severity_route` spells
/// (`posture/crates/posture-domain/src/severity.rs`). It is here because the
/// roster is what grants a route a key, and a route pns has no key for is a
/// posture page pns refuses to sign.
pub const POSTURE_ROUTE: &str = "posture";

/// Every route pns posts to, each verified by its OWN signing key: one
/// compromised key reaches one Discord channel rather than all of them.
///
/// A ROUTE NOT IN THIS LIST HAS NO KEY, and a post to one is refused rather
/// than signed with somebody else's: `--channel` takes any usable name, so
/// falling back to a shared key would sign for a route nobody granted.
pub const ROUTES: &[&str] = &[
    DEFAULT_ROUTE,
    RECAP_ROUTE,
    POSTURE_ROUTE,
    crate::stale::PRIORITY_ROUTE,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_name_can_stand_as_a_url_path_segment() {
        // The roster is what the schema admits, so a name that could not
        // become a path segment would be a key an operator may write and no
        // post could ever use.
        for route in ROUTES {
            assert!(
                crate::safety::route_name_is_usable(route),
                "`{route}` cannot stand as a route"
            );
        }
    }

    #[test]
    fn the_roster_names_each_route_once() {
        let mut sorted: Vec<&&str> = ROUTES.iter().collect();
        sorted.sort_unstable();
        let mut unique = sorted.clone();
        unique.dedup();
        assert_eq!(sorted, unique, "a duplicated route is a duplicated key");
    }
}
