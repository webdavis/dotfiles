//! A loopback double for the webhook gateway: the smallest thing that reads
//! the real bytes one signed POST puts on a socket.
//!
//! IT EXISTS BECAUSE HEADERS ARE NOT OBSERVABLE THROUGH THE SEAM. The
//! `SignedPost` trait carries a body, a signature and a request id as
//! arguments, so a scripted double proves what the sink asked for and nothing
//! about what the production client actually sent. `X-Request-ID` is the
//! header the gateway reads to recognize a retry, so a test that cannot see it
//! leaves the one line that arms that recognition unguarded.
//!
//! It binds port zero on loopback and answers every connection itself, so it
//! reaches no real gateway, no real route and no network.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long the double waits for the requests it was told to expect. Bounded
/// on purpose: a subject that posts nothing must fail the assertion rather
/// than park the test in `accept`.
const DEADLINE: Duration = Duration::from_secs(5);

pub(crate) struct CaptureGateway {
    base_url: String,
    reader: JoinHandle<Vec<String>>,
}

impl CaptureGateway {
    /// A gateway that answers `204` to `expected` requests and records the
    /// request line and headers of each.
    pub(crate) fn expecting(expected: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let base_url = format!("http://{}/webhooks", listener.local_addr().unwrap());
        listener.set_nonblocking(true).expect("a bounded accept");
        let reader = std::thread::spawn(move || {
            let mut seen = Vec::new();
            let until = Instant::now() + DEADLINE;
            while seen.len() < expected && Instant::now() < until {
                match listener.accept() {
                    Ok((stream, _)) => seen.push(answer(stream)),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
            seen
        });
        Self { base_url, reader }
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The head of every request that arrived, in order.
    pub(crate) fn requests(self) -> Vec<String> {
        self.reader.join().expect("the capture thread")
    }
}

/// Read one request's head, answer it, and hand the head back. The body is
/// never read: the signature and the request id are both in the head, and
/// answering without draining the body is what keeps this to one small read.
fn answer(stream: TcpStream) -> String {
    // An accepted socket inherits the listener's non-blocking mode on this
    // platform, and a non-blocking read answers WouldBlock instead of the
    // request, so the head has to be read on a blocking socket with a
    // deadline of its own.
    stream.set_nonblocking(false).expect("a blocking read");
    stream
        .set_read_timeout(Some(DEADLINE))
        .expect("a bounded read");
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line == "\r\n" => break,
            Ok(_) => head.push_str(&line),
            Err(_) => break,
        }
    }
    let stream = reader.into_inner();
    let mut writer = &stream;
    let _ = writer.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    let _ = writer.flush();
    let _ = stream.shutdown(Shutdown::Both);
    head
}
