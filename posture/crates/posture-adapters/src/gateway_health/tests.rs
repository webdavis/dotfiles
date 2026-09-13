use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
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
                        thread::yield_now()
                    }
                    Err(_) => return,
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            stream.write_all(format!("HTTP/1.1 {status} Fixture\r\nLocation: http://127.0.0.1:1/must-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
            send.send(request).unwrap();
        });
        let mut probe = GatewayProbe::new(
            format!("http://{address}/webhooks/priority"),
            Duration::from_millis(200),
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
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = wait.recv_timeout(Duration::from_millis(350));
                    drop(stream);
                    return;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::yield_now()
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
        elapsed < Duration::from_millis(180),
        "probe took {elapsed:?}"
    );
}
