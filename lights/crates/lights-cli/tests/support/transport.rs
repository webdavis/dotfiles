use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use ureq::unversioned::{
    resolver::{ResolvedSocketAddrs, Resolver},
    transport::{Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport},
};

#[derive(Debug)]
pub struct ScriptedConnector {
    responses: Mutex<VecDeque<Vec<u8>>>,
    pub requests: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl ScriptedConnector {
    pub fn new(responses: Vec<(u16, serde_json::Value)>) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(status, value)| {
                        let body = value.to_string();
                        format!(
                            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n\r\n{body}",
                            body.len()
                        )
                        .into_bytes()
                    })
                    .collect(),
            ),
            requests: Arc::default(),
        }
    }
}
impl Connector for ScriptedConnector {
    type Out = ScriptedTransport;
    fn connect(
        &self,
        details: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        assert_eq!(details.uri.scheme_str(), Some("https"));
        assert!(details.config.proxy().is_none());
        assert_eq!(details.config.max_redirects(), 0);
        assert!(!details.timeout.after.is_not_happening());
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected request");
        let mut requests = self.requests.lock().unwrap();
        let index = requests.len();
        requests.push(vec![]);
        Ok(Some(ScriptedTransport {
            buffers: LazyBuffers::new(65536, 65536),
            response,
            offset: 0,
            requests: self.requests.clone(),
            index,
        }))
    }
}
#[derive(Debug)]
pub struct ScriptedTransport {
    buffers: LazyBuffers,
    response: Vec<u8>,
    offset: usize,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    index: usize,
}
impl Transport for ScriptedTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, amount: usize, _: NextTimeout) -> Result<(), ureq::Error> {
        self.requests.lock().unwrap()[self.index]
            .extend_from_slice(&self.buffers.output()[..amount]);
        Ok(())
    }
    fn await_input(&mut self, _: NextTimeout) -> Result<bool, ureq::Error> {
        let pending = &self.response[self.offset..];
        let sink = self.buffers.input_append_buf();
        let amount = pending.len().min(sink.len());
        sink[..amount].copy_from_slice(&pending[..amount]);
        self.buffers.input_appended(amount);
        self.offset += amount;
        Ok(amount > 0)
    }
    fn is_open(&mut self) -> bool {
        self.offset < self.response.len()
    }
    fn is_tls(&self) -> bool {
        true
    }
}
#[derive(Debug)]
pub struct ScriptedResolver;
impl Resolver for ScriptedResolver {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut addresses = self.empty();
        addresses.push("192.0.2.1:443".parse().unwrap());
        Ok(addresses)
    }
}

#[derive(Debug)]
pub struct TimeoutConnector;
impl Connector for TimeoutConnector {
    type Out = ScriptedTransport;
    fn connect(
        &self,
        details: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Err(ureq::Error::Timeout(details.timeout.reason))
    }
}
