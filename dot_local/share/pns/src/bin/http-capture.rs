//! Test support: a one-shot HTTP capture server for the integration gate.
//!
//! Binds an ephemeral loopback port, writes the port number to argv[1],
//! accepts ONE request, writes its raw bytes (headers and body) to argv[2],
//! answers 200, and exits. std only, built alongside the engine, so the gate
//! depends on nothing with an interpreter cold start to diagnose. Exits
//! non-zero if no request arrives within thirty seconds, so a wedged gate
//! fails instead of hanging.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(port_path), Some(capture_path)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: http-capture <port-file> <capture-file> [status]");
        std::process::exit(2);
    };
    // An optional status lets the gate play an unhappy gateway.
    let status: u16 = arguments
        .next()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(200);

    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let port = listener.local_addr().expect("local addr").port();
    std::fs::write(&port_path, port.to_string()).expect("write port file");

    listener.set_nonblocking(true).expect("nonblocking");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    eprintln!("http-capture: no request within the deadline");
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

    // Read headers, then exactly Content-Length body bytes: reading to EOF
    // would deadlock against a client that waits for the response.
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

    std::fs::write(&capture_path, &raw).expect("write capture");
    let _ =
        stream.write_all(format!("HTTP/1.1 {status} X\r\nContent-Length: 0\r\n\r\n").as_bytes());
}
