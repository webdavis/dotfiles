//! The scripted HTTP transport every native leg's tests run over.
//!
//! THE `URLProtocolStub` MOVE, IN RUST, and SHARED because two adapters need
//! it: `ureq`'s `Agent::with_parts` accepts a bespoke `Connector`, so the REAL
//! agent pipeline runs (URL building, the headers, the redirect policy, the
//! body cap) and only the wire is scripted. Nothing here reaches the network.
//!
//! The seam lives in `ureq::unversioned`, which is exempt from semver;
//! `Cargo.lock` pins the version, so it can only shift the day ureq is
//! deliberately bumped, with the tests over this here to catch it.

use std::sync::{Arc, Mutex};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
};

/// Hands out one scripted response per connection and keeps a shared
/// capture of every byte the adapter transmits.
#[derive(Debug, Default)]
pub(crate) struct ScriptedConnector {
    pub(crate) wire: Arc<Mutex<Vec<u8>>>,
    pub(crate) responses: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
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
pub(crate) struct ScriptedTransport {
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

/// One scripted `200 OK` carrying this JSON body.
pub(crate) fn http_ok(body: &str) -> Vec<u8> {
    http_response("200 OK", &[("content-type", "application/json")], body)
}

/// One scripted response: a status line, the headers to send, and a body.
pub(crate) fn http_response(status: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
    format!("{head}{body}").into_bytes()
}

/// Resolves EVERY host to the loopback address.
///
/// IT IS WHAT MAKES A REDIRECT TEST REAL. Under the default resolver a
/// redirect to a name that does not exist fails at DNS, so a test asserting
/// "the second host was never contacted" passes whatever the redirect policy
/// says: verified 2026-09-15, where `max_redirects(5)` left that assertion
/// green. With this, a followed redirect connects, transmits, and shows up in
/// the capture.
#[derive(Debug, Default)]
pub(crate) struct LoopbackResolver;

impl ureq::unversioned::resolver::Resolver for LoopbackResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        _config: &ureq::config::Config,
        _timeout: NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let port = uri.port_u16().unwrap_or(80);
        let loopback = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        // `from_fn` sizes the array and leaves it EMPTY; `push` is what puts
        // an address in it, which is how ureq's own resolver builds one.
        let mut resolved = ureq::unversioned::resolver::ResolvedSocketAddrs::from_fn(|_| loopback);
        resolved.push(loopback);
        Ok(resolved)
    }
}
