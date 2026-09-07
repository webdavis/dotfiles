use super::parse::first_site_id;
use pns_application::Router;

/// The production router client, over the same HTTP stack as the other
/// native legs.
///
/// THE SECRET'S PATH IS THE POINT, exactly as in the moshi channel: the key
/// travels from the config into the request HEADER and nowhere else,
/// never argv, never a child's environment, never an error string. TLS
/// verification is disabled the way the hue bridge's is, and for the same
/// reason: the router serves a self-signed certificate for its own LAN
/// address, and no CA vouches for it.
pub struct UniFiRouter {
    /// The agent every call rides, INJECTED so a test can hand in one wearing
    /// a scripted transport (`Agent::with_parts`): the production pipeline
    /// runs for real and only the wire is fake, which is the closest Rust
    /// analog to stubbing Swift's URL Loading System.
    agent: ureq::Agent,
    /// e.g. `https://192.168.1.1`, from the `[plugins.router]` table.
    base: String,
    /// The API key, from the `[plugins.router]` table.
    key: String,
}

/// One bounded fetch: a probe on a diagnostic path is worth seconds, never a
/// hang.
const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// The router's answers are small (a 200-client listing measures kilobytes),
/// so the read is capped far below ureq's own 10MB default: a faulty router
/// streaming garbage costs at most this much memory before reading Unknown.
const ROUTER_BODY_CAP: u64 = 1_000_000;

impl UniFiRouter {
    /// The production wiring: TLS with verification disabled exactly as the
    /// hue bridge's is (the router serves a self-signed certificate for its
    /// own LAN address, and no CA vouches for it), no redirects, and the
    /// deadline on every call.
    pub fn new(base: String, key: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(ROUTER_DEADLINE))
            .max_redirects(0)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent();
        Self::with_agent(agent, base, key)
    }

    /// The same router over any agent: the seam the scripted-transport tests
    /// inject through.
    pub fn with_agent(agent: ureq::Agent, base: String, key: String) -> Self {
        Self { agent, base, key }
    }
}

/// The integration API's clients listing for the default site.
///
/// The site is resolved by name (`default`) through the sites listing first,
/// because site ids are per-install; both calls ride one agent and one
/// deadline each.
impl Router for UniFiRouter {
    fn clients_json(&self) -> Option<String> {
        let get = |path: &str| {
            self.agent
                .get(format!("{}{path}", self.base))
                .header("X-API-KEY", &self.key)
                .call()
                .ok()?
                .body_mut()
                .with_config()
                .limit(ROUTER_BODY_CAP)
                .read_to_string()
                .ok()
        };
        let sites = get("/proxy/network/integration/v1/sites")?;
        let site = first_site_id(&sites)?;
        get(&format!(
            "/proxy/network/integration/v1/sites/{site}/clients?limit=200"
        ))
    }
}
