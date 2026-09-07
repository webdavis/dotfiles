//! Test support: a one-shot HTTP capture server for the integration gate.
//!
//! Binds an ephemeral loopback port, writes the port number to `argv[1]`,
//! accepts one request (or as many as `argv[4]` asks for), APPENDS each one's
//! raw bytes (headers and body) to `argv[2]`, answers 200, and exits. std only,
//! built alongside the engine, so the gate depends on nothing with an
//! interpreter cold start to diagnose. Exits non-zero if the requests do not
//! all arrive within thirty seconds, so a wedged gate fails instead of hanging.
//!
//! MORE THAN ONE REQUEST IS A FALLBACK'S ONLY WITNESS. A leg that posts to a
//! route, is refused, and posts the same body to another one is two requests to
//! the SAME host and port, so two servers cannot observe it and one that exited
//! after the first cannot either.
//!
//! IT ALSO STANDS IN FOR AN HTTP PROXY, which is how a gate observes the URL
//! an event was posted to without owning the port in that URL. ureq speaks
//! CONNECT to an HTTP proxy rather than sending an absolute-form request
//! line, so the request being captured only exists past that handshake: the
//! CONNECT is answered and the SAME socket is read again. NOTHING IS EVER
//! FORWARDED. The tunnel terminates here, no connection is opened to the
//! target named in the CONNECT, and a gate that pointed this at a real
//! endpoint would still reach nothing.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(port_path), Some(capture_path)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: http-capture <port-file> <capture-file> [status] [requests]");
        std::process::exit(2);
    };
    // An optional status lets the gate play an unhappy gateway.
    let status: u16 = arguments
        .next()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(200);
    // And an optional count lets it observe a leg that posts more than once.
    let requests: usize = arguments
        .next()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(1);

    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let port = listener.local_addr().expect("local addr").port();
    std::fs::write(&port_path, port.to_string()).expect("write port file");
    // TRUNCATED ONCE, HERE, because each request is appended below: a file an
    // earlier run left behind must not read as this run's first request.
    std::fs::write(&capture_path, b"").expect("write capture");

    listener.set_nonblocking(true).expect("nonblocking");
    // ONE DEADLINE FOR THE WHOLE RUN rather than one per request, so a gate
    // expecting two and given one fails at the same bound as a gate given
    // none, instead of doubling how long a wedged run parks the suite.
    let deadline = Instant::now() + Duration::from_secs(30);
    for _ in 0..requests {
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        eprintln!("http-capture: too few requests within the deadline");
                        std::process::exit(1);
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => {
                    eprintln!("http-capture: accept failed: {error}");
                    std::process::exit(1);
                }
            }
        };

        stream.set_nonblocking(false).expect("blocking stream");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("read timeout");

        let mut raw = read_message(&mut stream);
        if raw.starts_with(b"CONNECT ") {
            let _ = stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
            raw = read_message(&mut stream);
        }

        // A NEWLINE BETWEEN MESSAGES AND NEVER AFTER ONE. An HTTP body does not
        // end in a newline, so a second message appended raw would weld its
        // request line onto the end of the first message's body and a gate
        // reading request lines would see one request. Written only between,
        // so a single-request capture is byte for byte what it always was.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&capture_path)
            .and_then(|mut capture| {
                if capture.metadata()?.len() > 0 {
                    capture.write_all(b"\n")?;
                }
                capture.write_all(&raw)
            })
            .expect("write capture");
        let _ = stream
            .write_all(format!("HTTP/1.1 {status} X\r\nContent-Length: 0\r\n\r\n").as_bytes());
    }
}

/// One HTTP message off the socket: headers to the blank line, then exactly
/// Content-Length body bytes. Reading to EOF would deadlock against a client
/// that waits for the response.
fn read_message(stream: &mut TcpStream) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break raw.len();
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(position) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let content_length = String::from_utf8_lossy(&raw[..header_end])
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    while raw.len() < header_end + content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    raw
}
