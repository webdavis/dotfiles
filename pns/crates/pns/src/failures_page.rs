//! The failure record, served as a page for moshi's browser preview.
//!
//! LOOPBACK ONLY, and that is the whole of the access control. The phone reaches
//! this over a per-session SSH local-forward through a terminal session that is
//! already authenticated, so the forward is the trust boundary and nothing needs
//! to be reachable on any other interface. Binding anywhere else would put an
//! unauthenticated page carrying route names and producer commands on the
//! network.
//!
//! EVERY FIELD VALUE IS THE TERMINAL'S OWN, produced by the same functions
//! `pns failures` prints from and laid out here for a phone rather than
//! reformatted. A page that computed its own status word or its own clock
//! would be a second formatter free to drift from the one an operator reads
//! at their desk, and the two would then disagree about the same failure.
//!
//! IT IS READ-ONLY AND HAS NO ROUTES BUT TWO. `/` is the listing and `/<id>` is
//! one record; everything else is a 404. There is no acknowledgement, no delete
//! and no query string, because a page reachable from a phone over a tunnel is
//! not where an irreversible action belongs.

use pns_adapters::SqliteStore;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

/// How many rows the page lists, matching the terminal view's own limit.
const LISTING_LIMIT: u32 = 20;

/// How long the child waits before trying a taken port again. See [`bind`].
const REBIND_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// How long the refusal above stays said before it is written again. See
/// [`say_now`].
const RESAY_AFTER: std::time::Duration = std::time::Duration::from_secs(600);

/// How long `answer` waits for a request line before giving up on the client.
///
/// THE LOOP IS SINGLE THREADED, so a connection that never sends a line would
/// otherwise block every reader behind it forever: a stray preconnect from a
/// browser is enough to leave the page silently dead until the daemon that
/// started it restarts it.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Serve until the listener dies, which for the daemon's child means until the
/// daemon stops it.
///
/// SINGLE THREADED ON PURPOSE. One operator with one phone reads one page at a
/// time, and a thread per connection would be machinery guarding against a load
/// this cannot have.
pub(crate) fn serve(port: u16) {
    parent_watch::exit_when_orphaned();
    serve_on(bind(port));
}

/// The serving half, over a listener somebody else opened.
///
/// SPLIT FROM THE BIND so a test can hand it an OS-ASSIGNED port. A test that
/// named the shipped port would be racing every other test in its suite for one
/// fixed number, and the retry loop above would turn that race into a
/// thirty-second wait rather than a failure.
fn serve_on(listener: TcpListener) {
    let store = SqliteStore::for_records(pns_adapters::state_dir());
    serve_on_within(listener, &store, REQUEST_TIMEOUT);
}

/// `serve_on`, with the store and the request timeout named rather than
/// resolved from the real state dir and a constant. A test hands this a
/// store of its own over a scratch path, never the operator's real ledger,
/// and can shrink the timeout to prove the loop moves on inside milliseconds
/// rather than waiting out the production value.
fn serve_on_within(
    listener: TcpListener,
    store: &SqliteStore,
    request_timeout: std::time::Duration,
) {
    // `flatten` DROPS THE FAILED ACCEPTS, which is the point: a phone that hung
    // up mid-handshake must not take the page down for the next reader.
    for stream in listener.incoming().flatten() {
        let _ = answer(store, stream, request_timeout);
    }
}

/// The listener, waiting for the port if something else holds it.
///
/// THE WAIT LIVES HERE RATHER THAN IN THE DAEMON, and that is what keeps the
/// daemon quiet. A child that exited on a taken port would be restarted every
/// tick, and each restart would write the same line: three to five times a
/// minute, forever, into the file the log rotation reads. Waiting inside one
/// child turns that into ONE line and a process that binds the moment the port
/// is free.
///
/// IT NEVER GIVES UP, because a taken port is a condition that heals: the
/// process holding it exits, or the operator moves it. Exiting would leave the
/// page permanently off with nothing to notice it and no second line to say so.
fn bind(port: u16) -> TcpListener {
    let mut said = None;
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return listener,
            Err(error) => {
                let now = std::time::Instant::now();
                if say_now(said, now) {
                    eprintln!(
                        "pns failures: could not bind 127.0.0.1:{port}: {error}; \
                         waiting for the port"
                    );
                    said = Some(now);
                }
                std::thread::sleep(REBIND_AFTER);
            }
        }
    }
}

/// Whether the refusal above is written on this attempt.
///
/// ONCE, THEN EVERY `RESAY_AFTER`. A line per retry wrote sixteen thousand
/// copies of one sentence into the daemon's log; a line written once and never
/// again leaves a page that has been down for days saying so only in a file
/// that has since rotated. The repeat is what keeps a standing refusal
/// readable in the log the operator actually has.
fn say_now(said: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    said.is_none_or(|said| now.duration_since(said) >= RESAY_AFTER)
}

fn answer(
    store: &SqliteStore,
    mut stream: TcpStream,
    request_timeout: std::time::Duration,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(request_timeout))?;
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let now = pns_adapters::now_secs().unwrap_or(0);
    let response = match target(&line) {
        Some(Target::Listing) => ok(&listing(store, now)),
        Some(Target::One(id)) => ok(&one(store, id, now)),
        None => not_found(),
    };
    stream.write_all(response.as_bytes())
}

/// What the request line asks for, or `None` for anything this does not serve.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    Listing,
    One(u64),
}

/// The request line, parsed.
///
/// GET ONLY, and no query string. A method this does not serve and a path it
/// does not know are the same answer, because a page with two routes has nothing
/// to say about either.
fn target(line: &str) -> Option<Target> {
    let mut words = line.split_whitespace();
    if words.next()? != "GET" {
        return None;
    }
    match words.next()?.strip_prefix('/')? {
        "" => Some(Target::Listing),
        rest => rest.parse::<u64>().ok().map(Target::One),
    }
}

fn listing(store: &SqliteStore, now: u64) -> String {
    match store.failing_legs(LISTING_LIMIT) {
        Err(_) => html::sentence_page("pns: the delivery ledger could not be read"),
        Ok(failures) => {
            html::listing_page(&failures, &|id| store.retry_facts(id).ok().flatten(), now)
        }
    }
}

fn one(store: &SqliteStore, id: u64, now: u64) -> String {
    match store.failing_leg(id) {
        Err(_) => html::sentence_page("pns: the delivery ledger could not be read"),
        Ok(None) => html::sentence_page(&format!("pns: no failure {id}")),
        Ok(Some(stored)) => {
            let install =
                pns_adapters::install_settings(&std::env::var("HOME").unwrap_or_default());
            let failure = crate::command_failures::compose(
                &stored,
                install.moshi_url.as_deref(),
                install.hermes_url.as_deref(),
            );
            let row = crate::command_failures::rows(std::slice::from_ref(&stored)).remove(0);
            let retry = store.retry_facts(id).ok().flatten();
            html::record_page(&failure, &row, retry, now)
        }
    }
}

/// A response, with the header `moshi-hook`'s probe looks for. `Content-Length`
/// is what lets a phone's browser finish the response rather than wait on a
/// connection this closes.
fn response(status: &str, body: String) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn ok(body: &str) -> String {
    response("200 OK", body.to_string())
}

fn not_found() -> String {
    response(
        "404 Not Found",
        html::sentence_page("pns: this page serves / and /<id>"),
    )
}

mod html;
mod parent_watch;

#[cfg(test)]
#[path = "failures_page/tests.rs"]
mod tests;
