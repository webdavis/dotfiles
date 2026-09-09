use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// How long the fixture server waits for the client at each step.
///
/// FIXTURE PATIENCE, not a measurement. Nothing here times anything: the
/// deadline only exists so a client that never arrives fails the test instead
/// of hanging it. It was 400 ms, a wall-clock budget for a loopback round trip
/// while the rest of the suite competes for the same CPU, and it expired on a
/// loaded machine. A generous bound costs the happy path nothing.
pub(super) const DEADLINE: Duration = Duration::from_secs(30);

pub(super) fn serve(listener: TcpListener, response: &str, body: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + DEADLINE;
    listener.set_nonblocking(true)?;
    let (mut stream, _) = poll(deadline, || listener.accept())?;
    stream.set_nonblocking(true)?;

    // Consume the entire known request before replying, so closing a socket
    // while the client still writes cannot reset an otherwise valid response.
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let read = poll(deadline, || stream.read(&mut chunk))?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the request ended before its body arrived",
            ));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > 8192 {
            return Err(io::Error::other("the fixture request exceeded 8192 bytes"));
        }
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            let received = &request[end + 4..];
            if received.len() >= body.len() {
                if received != body {
                    return Err(io::Error::other("the expected request body did not arrive"));
                }
                break;
            }
        }
    }

    let mut remaining = response.as_bytes();
    while !remaining.is_empty() {
        let written = poll(deadline, || stream.write(remaining))?;
        if written == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        remaining = &remaining[written..];
    }
    // Hold the socket until the client hangs up. Every drain uses the same
    // absolute deadline, so trickled bytes cannot renew the server's lifetime.
    while poll(deadline, || stream.read(&mut chunk))? > 0 {}
    Ok(())
}

fn poll<T>(deadline: Instant, mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the fixture did not finish within its deadline",
            ));
        }
        match operation() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            result => return result,
        }
    }
}
