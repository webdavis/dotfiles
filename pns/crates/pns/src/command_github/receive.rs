//! `pns github receive`: the push transport, which is a DOORBELL and not a
//! second source.
//!
//! WHAT ARRIVES ON THE SOCKET IS NEVER READ. A verified delivery means
//! something happened on GitHub, and the poll beside this is what reads WHAT:
//! one notification reader, one identity, one seen-set, so a push and the tick
//! after it cannot deliver the same event twice. The webhook body would be a
//! second mapping of GitHub's own taxonomy and a second spelling of every
//! identity, and the two would disagree the first time a payload carried an id
//! the notifications API states differently.
//!
//! THE POLL IS THE FLOOR. A receiver that is off, unreachable, refused at the
//! edge or crashed changes nothing: the scheduled job still runs, and the only
//! thing lost is the seconds this saves.

use super::{Launch, armed_source, poll_once};

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// `pns github receive`: bind loopback, and poll on every verified delivery.
///
/// LOOPBACK ONLY. The tunnel is the way in, and the tunnel already terminates
/// on this machine: a receiver bound to every interface would be reachable on
/// the local network as well, which nothing asked for.
pub(super) fn github_receive() -> i32 {
    let source = match armed_source() {
        Ok(source) => source,
        Err(code) => return code,
    };
    let Some(webhook) = source.webhook.as_ref() else {
        // INERT RATHER THAN A FAILURE, and it exits 0 so launchd's
        // `SuccessfulExit false` leaves it exited: a machine whose operator
        // has not created the GitHub App yet has a working poll and nothing
        // listening. SAID ON EVERY LAUNCH, unlike a poll's transient
        // complaints, because it is said once per launch into a log the
        // operator reads to find out why nothing is listening.
        println!(
            "pns github receive: no `webhook_secret` in [plugins.github], \
             so nothing is listening; the poll is the source either way"
        );
        return 0;
    };
    let Ok(port) = u16::try_from(webhook.port) else {
        // The config's own bounds make this unreachable; it is a refusal
        // rather than a truncating cast so a widened bound cannot turn into a
        // receiver listening on a port nobody named.
        eprintln!(
            "pns github receive: `webhook_port` {} is not a port",
            webhook.port
        );
        return 1;
    };
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "pns github receive: 127.0.0.1:{} could not be bound ({error})",
                webhook.port
            );
            return 1;
        }
    };
    println!(
        "pns github receive: listening on 127.0.0.1:{}{}",
        webhook.port,
        pns_adapters::WEBHOOK_PATH
    );
    for stream in listener.incoming() {
        // ONE CONNECTION AT A TIME, in arrival order. A delivery is answered
        // in the time one conditional request takes, GitHub's own deliveries
        // for one account arrive seconds apart, and a thread per connection
        // would be a thread per stranger on a path the internet can reach.
        match stream {
            Ok(stream) => answer(stream, &webhook.secret, &mut || {
                poll_once(&source, Launch::Daemon);
            }),
            // A FAILED ACCEPT IS NOT A DEAD RECEIVER: the listener is still
            // bound, so the next connection is tried rather than the process
            // exiting and burning a throttled restart.
            Err(error) => eprintln!("pns github receive: a connection failed ({error})"),
        }
    }
    0
}

/// One connection: read it, answer it, and ring the doorbell when it was
/// really GitHub.
///
/// THE ANSWER GOES OUT BEFORE THE POLL RUNS. GitHub gives a delivery ten
/// seconds and a poll is allowed the same ten, so a receiver that polled first
/// would record a timed-out delivery on GitHub's side for work it had already
/// done.
fn answer(mut stream: TcpStream, secret: &str, doorbell: &mut impl FnMut()) {
    // The read side is bounded inside read_request, one deadline for the
    // whole message; only the write needs setting here.
    let _ = stream.set_write_timeout(Some(REQUEST_DEADLINE));
    let verdict = match read_request(&mut stream) {
        Ok(request) => pns_adapters::delivery(&request, secret),
        Err(reason) => pns_adapters::Delivery::Refused(reason),
    };
    // A REAL REASON PHRASE, and no Content-Length on the 204: RFC 9110
    // forbids one on a response with no body, and "X" said nothing anyway.
    let response = match verdict {
        pns_adapters::Delivery::Verified => "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
        pns_adapters::Delivery::Refused(_) => {
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        }
    };
    let _ = stream.write_all(response.as_bytes());
    ring(verdict, doorbell);
}

/// The doorbell rung on a verified delivery, and a refusal said once in the
/// receiver's own log.
///
/// THE REASON IS NEVER IN THE RESPONSE: an answer that told the sender which
/// check it failed is an oracle for the next attempt, so every refusal leaves
/// by the one status and the reason stays here.
///
/// A DELIVERY IS ONE POLL, with no minimum gap between them.
/// ponytail: unthrottled, one conditional request per delivery against a
/// 5000-an-hour budget; a minimum gap belongs here the day a storm measures
/// close to it.
fn ring(verdict: pns_adapters::Delivery, doorbell: &mut impl FnMut()) {
    match verdict {
        pns_adapters::Delivery::Verified => doorbell(),
        pns_adapters::Delivery::Refused(reason) => {
            eprintln!("pns github receive: refused a request ({reason})");
        }
    }
}

/// How long one request may take to arrive. TEN SECONDS, the client's own
/// deadline: a connection that opens and says nothing must not hold the queue.
const REQUEST_DEADLINE: Duration = Duration::from_secs(10);

/// One HTTP message off the socket: the header block, then exactly as many
/// body bytes as it states.
///
/// THE CEILING IS APPLIED TO THE STATED LENGTH before anything is read, so a
/// number a stranger chose never decides this process's allocation. Reading to
/// end of file instead would deadlock against a client waiting for its answer.
fn read_request(stream: &mut TcpStream) -> Result<Vec<u8>, &'static str> {
    // ONE DEADLINE FOR THE WHOLE MESSAGE, not one per read: the socket's read
    // timeout resets on every call, so a client trickling in a byte just
    // under that timeout apart would otherwise hold the connection (and the
    // single-threaded accept loop) open indefinitely.
    let deadline = std::time::Instant::now() + REQUEST_DEADLINE;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        bound_read_timeout(stream, deadline)?;
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return Err("the connection said nothing");
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(head_end) = pns_adapters::head_end(&raw) {
            break head_end;
        }
        if raw.len() > HEAD_MAX {
            return Err("header block over the ceiling");
        }
    };
    // NO STATED LENGTH IS A REFUSAL, never zero: a body sent without
    // `Content-Length` (chunked framing, say) would otherwise be hashed as
    // empty, and a request that fails the signature check for that reason
    // logs the wrong cause.
    let Some(stated) = pns_adapters::content_length(&String::from_utf8_lossy(&raw[..head_end]))
    else {
        return Err("no stated content length");
    };
    if stated > pns_adapters::WEBHOOK_BODY_MAX {
        return Err("body over the ceiling");
    }
    while raw.len() < head_end + stated {
        bound_read_timeout(stream, deadline)?;
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    Ok(raw)
}

/// Shrinks the socket's read timeout to what is left of the request's total
/// budget, or refuses once nothing is left.
fn bound_read_timeout(
    stream: &TcpStream,
    deadline: std::time::Instant,
) -> Result<(), &'static str> {
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    if remaining.is_zero() {
        return Err("the request took too long to arrive");
    }
    let _ = stream.set_read_timeout(Some(remaining));
    Ok(())
}

/// The most header block this reads. Sixteen kibibytes, which is more than
/// GitHub sends and less than a stranger can grow without end.
const HEAD_MAX: usize = 16 * 1024;

#[cfg(test)]
#[path = "tests/answering.rs"]
mod answering_tests;
