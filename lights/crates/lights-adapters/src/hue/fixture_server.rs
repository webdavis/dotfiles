//! A loopback TLS server, so the pinned transport is measured against a real
//! handshake rather than against a stub that skips the part being tested.
//!
//! ITS CERTIFICATE HAS THE BRIDGE'S SHAPE: a synthetic bridge id as the common
//! name, `root-bridge` as the issuer, and NO subjectAltName, which is why no
//! trust anchor could stand in for the pin.

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The certificate the fixture serves, and the one a test pins to.
pub(super) const CERTIFICATE: &[u8] = include_bytes!("fixture_server/cert.der");

/// The bridge id the fixture's certificate names.
pub(super) const FIXTURE_ID: &str = "B0B0B0FFFE000001";

/// How long the fixture waits for a connection, and for any one socket read.
///
/// GENEROUS ON PURPOSE, and it costs nothing on the happy path: the accept loop
/// breaks the moment a connection arrives, so this bounds only the case where
/// the client never comes. A budget tight enough to be a TLS handshake's would
/// report "no request arrived" on a loaded machine and read as a pinning bug.
const PATIENCE: Duration = Duration::from_secs(10);

pub(super) struct Server {
    address: std::net::SocketAddr,
    thread: JoinHandle<Result<String, String>>,
}

impl Server {
    pub(super) fn start(body: &str) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        Self {
            address,
            thread: std::thread::spawn({
                let body = body.to_string();
                move || serve(listener, &body)
            }),
        }
    }
    /// The address a caller reaches it at, with no scheme, the way the config
    /// writes one.
    pub(super) fn address(&self) -> String {
        self.address.to_string()
    }
    /// The request that arrived, or why none did.
    pub(super) fn finish(self) -> Result<String, String> {
        self.thread.join().expect("the bounded fixture thread")
    }
}

fn serve(listener: TcpListener, body: &str) -> Result<String, String> {
    let deadline = Instant::now() + PATIENCE;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("no connection arrived".to_string());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(PATIENCE)).unwrap();
    stream.set_write_timeout(Some(PATIENCE)).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from(CERTIFICATE.to_vec())],
        PrivatePkcs8KeyDer::from(include_bytes!("fixture_server/key.der").to_vec()).into(),
    )
    .unwrap();
    let mut tls = rustls::StreamOwned::new(
        rustls::ServerConnection::new(Arc::new(config)).unwrap(),
        stream,
    );
    let request = request(&mut tls)?;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(response.as_bytes())
        .map_err(|error| error.to_string())?;
    tls.flush().map_err(|error| error.to_string())?;
    Ok(request)
}

/// The request as it arrived, headers and body both, so a test can assert what
/// was sent as well as that something was.
fn request(
    tls: &mut rustls::StreamOwned<rustls::ServerConnection, TcpStream>,
) -> Result<String, String> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        tls.read_exact(&mut byte).map_err(|e| e.to_string())?;
        bytes.push(byte[0]);
        if bytes.len() > 4096 {
            return Err("fixture request exceeds its limit".to_string());
        }
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8(bytes.clone()).map_err(|e| e.to_string())?;
    let length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    if length > 4096 {
        return Err("fixture body exceeds its limit".to_string());
    }
    let mut body = vec![0; length];
    tls.read_exact(&mut body).map_err(|e| e.to_string())?;
    bytes.extend(body);
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
