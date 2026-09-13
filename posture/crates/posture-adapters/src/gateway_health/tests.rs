use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

#[test]
fn an_unsigned_get_reports_all_http_statuses_without_following_redirects() {
    for status in [200, 204, 302, 404, 405, 503] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (send, receive) = mpsc::channel();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(500);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(_) => return,
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                assert!(read > 0, "client closed before completing its request");
                request.extend_from_slice(&chunk[..read]);
                assert!(request.len() < 8192);
            }
            stream.write_all(format!("HTTP/1.1 {status} Fixture\r\nLocation: http://127.0.0.1:1/must-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
            send.send(request).unwrap();
        });
        let mut probe = GatewayProbe::new(
            format!("http://{address}/webhooks/priority"),
            Duration::from_millis(500),
        );
        let observed = probe.status();
        server.join().unwrap();
        let request = receive.try_recv();
        assert_eq!(observed, Some(status));
        let request = String::from_utf8(request.unwrap()).unwrap().to_lowercase();
        assert!(request.starts_with("get /webhooks/priority http/1.1\r\n"));
        for forbidden in ["authorization:", "x-hermes", "signature:", "hmac"] {
            assert!(!request.contains(forbidden));
        }
    }
}
#[test]
fn an_unavailable_gateway_is_unknown_and_has_a_deadline() {
    let mut probe = GatewayProbe::new("http://127.0.0.1:1".into(), Duration::from_millis(20));
    let start = Instant::now();
    assert_eq!(probe.status(), None);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_gateway_that_accepts_but_never_answers_is_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (release, wait) = mpsc::channel();
    let expired = Arc::new(AtomicBool::new(false));
    let server_expired = Arc::clone(&expired);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if wait.recv_timeout(Duration::from_millis(700)).is_err() {
                        server_expired.store(true, Ordering::SeqCst);
                    }
                    drop(stream);
                    return;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return,
            }
        }
    });
    let mut probe = GatewayProbe::new(
        format!("http://{address}/priority"),
        Duration::from_millis(40),
    );
    let start = Instant::now();
    let status = probe.status();
    let elapsed = start.elapsed();
    let _ = release.send(());
    server.join().unwrap();
    assert_eq!(status, None);
    assert!(
        !expired.load(Ordering::SeqCst),
        "server closure ended the request instead of the client deadline"
    );
    assert!(elapsed < Duration::from_secs(1), "probe took {elapsed:?}");
}
