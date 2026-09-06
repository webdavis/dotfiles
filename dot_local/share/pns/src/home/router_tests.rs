//! The home probe, pinned: router.

use super::fixtures::*;

// --- pagination: an incomplete page must not report a departure ----------

#[test]
fn a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home() {
    // totalCount says 201 clients exist and the page carries one: the
    // phone may be among the 200 not shown, so absent-from-this-page must
    // not become NotHome, which is a false departure.
    assert_eq!(
        parse_clients(
            r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"dresden"}]}"#
        ),
        None
    );
}

#[test]
fn a_complete_page_with_unnamed_clients_still_answers() {
    // Completeness is judged on the ENTRIES: a page carrying as many as
    // the router counted is whole, and the unnamed entry is one of them.
    assert_eq!(
        parse_clients(r#"{"totalCount":2,"data":[{"id":"x"},{"name":"mister"}]}"#)
            .map(|clients| clients.len()),
        Some(2)
    );
}

#[test]
fn a_listing_without_total_count_is_taken_whole_as_before() {
    assert_eq!(
        parse_clients(r#"{"data":[{"name":"mister"}]}"#),
        only_names(&["mister"])
    );
}

// --- the site id, the one router answer that becomes part of a URL -------

#[test]
fn the_first_sites_id_is_extracted_from_the_live_shape() {
    assert_eq!(
        first_site_id(
            r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#
        ),
        Some("88f7af54-98f8-306a-a1c7-c9349722b1f6".to_string())
    );
}

#[test]
fn a_sites_answer_without_a_usable_id_is_no_answer() {
    for sites in [
        "",
        "not json",
        r#"{"data":[]}"#,
        r#"{"data":[{}]}"#,
        r#"{"data":[{"id":5}]}"#,
    ] {
        assert_eq!(first_site_id(sites), None, "case: {sites:?}");
    }
}

#[test]
fn an_id_that_could_escape_the_url_path_is_refused_outright() {
    // The id is joined into /sites/{id}/clients, so this is the trust
    // boundary: a corrupt or hostile router answer must never become a
    // path segment.
    for hostile in ["../evil", "a/b", "a?x=1", "a#frag", "id with space", ""] {
        assert_eq!(
            first_site_id(&format!(r#"{{"data":[{{"id":"{hostile}"}}]}}"#)),
            None,
            "case: {hostile:?}"
        );
    }
}

// --- the production adapter, over a scripted transport -------------------
//
// The URLProtocolStub move, in Rust: ureq's `Agent::with_parts` accepts a
// bespoke `Connector`, so the REAL agent pipeline runs (URL building, the
// header, redirect policy, the body cap) and only the wire is scripted.
// The seam lives in `ureq::unversioned`, which is exempt from semver;
// Cargo.lock pins the version, so it can only shift the day ureq is
// deliberately bumped, with these tests here to catch it.

use std::sync::{Arc, Mutex};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
};

/// Hands out one scripted response per connection and keeps a shared
/// capture of every byte the adapter transmits.
#[derive(Debug, Default)]
struct ScriptedConnector {
    wire: Arc<Mutex<Vec<u8>>>,
    responses: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
}

impl Connector for ScriptedConnector {
    type Out = ScriptedTransport;

    fn connect(
        &self,
        _details: &ConnectionDetails,
        _chained: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();
        Ok(Some(ScriptedTransport {
            buffers: LazyBuffers::new(65536, 65536),
            wire: Arc::clone(&self.wire),
            response,
            fed: 0,
        }))
    }
}

/// One connection: records what is transmitted, feeds the scripted
/// response in chunks, and refuses reuse so every request reconnects and
/// pops the next script entry.
#[derive(Debug)]
struct ScriptedTransport {
    buffers: LazyBuffers,
    wire: Arc<Mutex<Vec<u8>>>,
    response: Vec<u8>,
    fed: usize,
}

impl Transport for ScriptedTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, _timeout: NextTimeout) -> Result<(), ureq::Error> {
        let sent = self.buffers.output()[..amount].to_vec();
        self.wire.lock().unwrap().extend_from_slice(&sent);
        Ok(())
    }

    fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let pending = &self.response[self.fed..];
        if pending.is_empty() {
            return Ok(false);
        }
        let sink = self.buffers.input_append_buf();
        let amount = pending.len().min(sink.len());
        sink[..amount].copy_from_slice(&pending[..amount]);
        self.buffers.input_appended(amount);
        self.fed += amount;
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        // A finished response closes the connection, so the agent cannot
        // pool it: the next request reconnects and pops the next script.
        self.fed < self.response.len()
    }
}

/// The production config semantics (no redirects) over the scripted wire.
fn scripted_router(responses: &[Vec<u8>], key: &str) -> (super::UniFiRouter, Arc<Mutex<Vec<u8>>>) {
    let connector = ScriptedConnector::default();
    let wire = Arc::clone(&connector.wire);
    *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
    let config = ureq::Agent::config_builder().max_redirects(0).build();
    let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
    (
        super::UniFiRouter::with_agent(agent, "http://localhost:9".to_string(), key.to_string()),
        wire,
    )
}

fn http_ok(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

const SITES_CAPTURE: &str = r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#;

#[test]
fn the_adapter_sends_the_key_and_walks_sites_then_clients() {
    let (router, wire) =
        scripted_router(&[http_ok(SITES_CAPTURE), http_ok(CLIENTS_CAPTURE)], "k-123");
    assert_eq!(
        read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    );
    let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
    let sites_at = wire
        .find("get /proxy/network/integration/v1/sites http/1.1")
        .expect("the sites request was made");
    let clients_at = wire
        .find(
            "get /proxy/network/integration/v1/sites/88f7af54-98f8-306a-a1c7-c9349722b1f6/clients?limit=200 http/1.1",
        )
        .expect("the clients request was made with the extracted site id");
    assert!(sites_at < clients_at, "sites is asked before clients");
    assert!(
        wire.contains("x-api-key: k-123"),
        "the key rides the header: {wire}"
    );
}

#[test]
fn a_redirecting_router_is_never_followed_and_reads_unknown() {
    // max_redirects(0) is production config: a router answering 301 must
    // not send the adapter (and its key header) to the Location target.
    let (router, wire) = scripted_router(
        &[
            b"HTTP/1.1 301 Moved Permanently\r\nlocation: http://elsewhere.test/\r\ncontent-length: 0\r\n\r\n".to_vec(),
        ],
        "k-123",
    );
    assert_eq!(
        read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
        HomePresence::Unknown
    );
    let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
    assert_eq!(wire.matches("get ").count(), 1, "no second request: {wire}");
    assert!(
        !wire.contains("elsewhere"),
        "the redirect target is never contacted"
    );
}

#[test]
fn a_body_past_the_cap_reads_unknown_rather_than_being_swallowed() {
    // The oversized answer is VALID and would parse into Home if it were
    // read: that is what makes this pin the 1MB cap itself rather than
    // ureq's own 10MB default, which an unparseable body cannot tell
    // apart (a garbage body reads Unknown under any cap).
    let oversized_but_valid_sites = format!(
        r#"{{"padding":"{}","data":[{{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6"}}]}}"#,
        "x".repeat(1_100_000)
    );
    let (router, _wire) = scripted_router(
        &[
            http_ok(&oversized_but_valid_sites),
            http_ok(CLIENTS_CAPTURE),
        ],
        "k-123",
    );
    assert_eq!(
        read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
        HomePresence::Unknown
    );
}

#[test]
fn an_unknown_type_with_control_bytes_is_escaped_like_every_other_spelled_value() {
    let line = setup_report(&SetupFailure::UnknownType("a\u{1b}[31mz".to_string()));
    assert!(
        !line.contains('\u{1b}'),
        "raw ESC must not reach stdout: {line}"
    );
    assert!(
        line.contains("\\u{1b}"),
        "the escaped form is shown: {line}"
    );
}
