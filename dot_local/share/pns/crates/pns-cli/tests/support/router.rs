use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A listing where the keys DISAGREE: the MAC names the phone, the client
/// name matches nobody, and the address is now the neighbouring client's
/// lease. Shared, so "stale" is one fixture rather than one per test file.
pub const KEYS_DISAGREE: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mouse","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;

/// The `[plugins.router]` table KEYS_DISAGREE is read against, and the lines
/// that reading prints. Shared, because a second test asserting the
/// diagnostic is unchanged is only worth anything if "unchanged" is the same
/// text. LAST in every config built on it, so a test can append one more
/// router setting by writing one more line.
pub fn router_table(router_url: &str) -> String {
    format!(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"{router_url}\"\n\
         device_mac = \"2e:11:ab:6d:b0:4f\"\ndevice_hostname = \"mister-2\"\n\
         device_ipv4 = \"192.168.1.248\"\napi_key = \"k-123\"\n"
    )
}

/// A UniFi router on loopback, answering the two calls the probe makes: the
/// sites listing, then whatever clients listing it has been given.
///
/// IN-PROCESS, on a thread rather than a child: the engine under test is the
/// process that has to be real, and a listener the test owns can have its
/// listing swapped between runs, which is how a resolved staleness is
/// observed. The thread ends with the test binary.
pub struct RouterStub {
    port: u16,
    listing: Arc<Mutex<String>>,
}

impl RouterStub {
    pub fn start(listing: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        let port = listener.local_addr().expect("local addr").port();
        let listing = Arc::new(Mutex::new(listing.to_string()));
        let served = Arc::clone(&listing);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &served);
            }
        });
        RouterStub { port, listing }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The same listener addressed by NAME instead of by literal address.
    ///
    /// A proxy bypass matches on HOST ALONE, port ignored, so a test that
    /// routes the gateway through a proxy and needs this stub reached
    /// directly cannot tell the two apart while both are spelled
    /// `127.0.0.1`. `localhost` resolves here and is a different host string,
    /// which is the only handle the bypass rule offers.
    pub fn localhost_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// What the router says from the next call on.
    pub fn set_listing(&self, listing: &str) {
        *self.listing.lock().expect("the listing") = listing.to_string();
    }
}

/// One request, one answer, one closed connection: `Connection: close` keeps
/// the client from pooling a socket this server has already dropped.
///
/// THE WHOLE HEADER IS READ BEFORE IT IS ROUTED, the way `http-capture` reads
/// its own request. ONE `read` is not a request: a segment boundary landing
/// before the request line, or a read that errors, leaves the routing text
/// short or empty, and text carrying no "clients" serves the SITES body as
/// the clients listing. `parse_clients` accepts that as a complete listing of
/// one anonymous client, so the test reports NotHome and reads as a flake
/// rather than as its own assertion.
fn answer(mut stream: std::net::TcpStream, listing: &Mutex<String>) {
    // A client that opens a socket and says nothing must not park this
    // thread: the accept loop is serial, so one hang would stall every later
    // request instead of failing one.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match std::io::Read::read(&mut stream, &mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let body = if String::from_utf8_lossy(&request).contains("clients") {
        listing.lock().expect("the listing").clone()
    } else {
        r#"{"data":[{"id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}]}"#.to_string()
    };
    let _ = std::io::Write::write_all(
        &mut stream,
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}
