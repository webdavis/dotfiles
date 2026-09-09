//! The failure record, served as a page for moshi's browser preview.
//!
//! LOOPBACK ONLY, and that is the whole of the access control. The phone reaches
//! this over a per-session SSH local-forward through a terminal session that is
//! already authenticated, so the forward is the trust boundary and nothing needs
//! to be reachable on any other interface. Binding anywhere else would put an
//! unauthenticated page carrying route names and producer commands on the
//! network.
//!
//! IT SERVES WHAT THE TERMINAL PRINTS, character for character, inside a `<pre>`
//! block. A page that reformatted the record would be a second formatter free to
//! drift from the one an operator reads at their desk, and the two would then
//! disagree about the same failure.
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

/// Serve until the listener dies, which for the daemon's child means until the
/// daemon stops it.
///
/// SINGLE THREADED ON PURPOSE. One operator with one phone reads one page at a
/// time, and a thread per connection would be machinery guarding against a load
/// this cannot have.
pub(crate) fn serve(port: u16) {
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
    // `flatten` DROPS THE FAILED ACCEPTS, which is the point: a phone that hung
    // up mid-handshake must not take the page down for the next reader.
    for stream in listener.incoming().flatten() {
        let _ = answer(&store, stream);
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
    let mut said = false;
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return listener,
            Err(error) => {
                if !said {
                    eprintln!(
                        "pns failures: could not bind 127.0.0.1:{port}: {error}; \
                         waiting for the port"
                    );
                    said = true;
                }
                std::thread::sleep(REBIND_AFTER);
            }
        }
    }
}

fn answer(store: &SqliteStore, mut stream: TcpStream) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let response = match target(&line) {
        Some(Target::Listing) => ok(&listing(store)),
        Some(Target::One(id)) => ok(&one(store, id)),
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

fn listing(store: &SqliteStore) -> String {
    match store.failing_legs(LISTING_LIMIT) {
        Err(_) => "pns: the delivery ledger could not be read\n".to_string(),
        Ok(failures) => crate::command_failures::listing(&failures),
    }
}

fn one(store: &SqliteStore, id: u64) -> String {
    match store.failing_leg(id) {
        Err(_) => "pns: the delivery ledger could not be read\n".to_string(),
        Ok(None) => format!("pns: no failure {id}\n"),
        Ok(Some(stored)) => pns_domain::failure::full(&crate::command_failures::compose(&stored)),
    }
}

/// The record inside a page.
///
/// EVERYTHING IS ESCAPED. A route name and a producer command are producer text,
/// and a page is the one surface where producer text could otherwise become
/// markup. `<pre>` is what preserves the full form's column alignment, which is
/// the reason the record is laid out in columns at all.
fn page(body: &str) -> String {
    format!(
        "<!doctype html>\n<title>pns failures</title>\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <pre>{}</pre>\n",
        escaped(body)
    )
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    response("200 OK", page(body))
}

fn not_found() -> String {
    response("404 Not Found", page("pns: this page serves / and /<id>\n"))
}

#[cfg(test)]
#[path = "failures_page/tests.rs"]
mod tests;
