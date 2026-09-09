use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) enum Reply {
    Body,
    Redirect(String),
    Wait,
}

pub(super) struct Server {
    address: std::net::SocketAddr,
    thread: JoinHandle<Result<String, String>>,
}

impl Server {
    pub(super) fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || serve(listener, reply));
        Self { address, thread }
    }

    pub(super) fn url(&self) -> String {
        format!("https://{}", self.address)
    }

    pub(super) fn finish(self) -> Result<String, String> {
        self.thread.join().expect("the bounded fixture thread")
    }
}

/// How long the fixture waits for a connection, and how long it spends on any
/// one socket read or write.
///
/// GENEROUS ON PURPOSE, and it costs nothing on the happy path: the accept loop
/// breaks the moment a connection arrives, so this bounds only the case where
/// the client never comes. It used to be 250 ms, which is a budget for the
/// client's TLS handshake rather than for a hung fixture, and 700 other tests
/// share this machine. When the handshake lost that race the fixture reported
/// "no request arrived" and the test blamed the bridge.
const FIXTURE_PATIENCE: Duration = Duration::from_secs(10);

fn serve(listener: TcpListener, reply: Reply) -> Result<String, String> {
    let deadline = Instant::now() + FIXTURE_PATIENCE;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("no request arrived".to_string());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(FIXTURE_PATIENCE)).unwrap();
    stream.set_write_timeout(Some(FIXTURE_PATIENCE)).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from(include_bytes!("cert.der").to_vec())],
        PrivatePkcs8KeyDer::from(include_bytes!("key.der").to_vec()).into(),
    )
    .unwrap();
    let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
    let mut tls = rustls::StreamOwned::new(connection, stream);
    let request = request(&mut tls)?;
    let response = match reply {
        Reply::Body => {
            "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"data\":[]}"
                .to_string()
        }
        Reply::Redirect(location) => format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        // Never answer, and hold the connection until the CLIENT lets go. This
        // used to sleep a fixed 400 ms, which every run of the deadline test
        // paid in full because finishing the fixture joins this thread, and
        // which put a hard ceiling on how much headroom that test's deadline
        // could be given. Waiting for the peer instead ends the moment the
        // caller's own deadline fires, so the test costs its deadline and not a
        // constant, and a bridge that never gives up is bounded by
        // `FIXTURE_PATIENCE` rather than answered early.
        Reply::Wait => {
            let mut ignored = [0; 1];
            let _ = tls.read(&mut ignored);
            return Ok(request);
        }
    };
    tls.write_all(response.as_bytes())
        .map_err(|e| e.to_string())?;
    tls.flush().map_err(|e| e.to_string())?;
    Ok(request)
}

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
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    if content_length > 4096 {
        return Err("fixture body exceeds its limit".to_string());
    }
    let mut body = vec![0; content_length];
    tls.read_exact(&mut body).map_err(|e| e.to_string())?;
    bytes.extend(body);
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
