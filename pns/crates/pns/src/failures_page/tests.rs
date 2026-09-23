use super::*;

/// The three routes the page serves, and nothing else needs a name.
#[test]
fn root_is_the_index_failures_is_the_listing_and_a_number_under_it_is_one_record() {
    assert_eq!(target("GET / HTTP/1.1\r\n"), Some(Target::Index));
    assert_eq!(target("GET /failures HTTP/1.1\r\n"), Some(Target::Listing));
    assert_eq!(
        target("GET /failures/47 HTTP/1.1\r\n"),
        Some(Target::One(47))
    );
}

/// A page with three routes has nothing to say about a fourth, so a path it
/// does not know is refused rather than guessed at. The bare `/<id>` this
/// page used to serve is deliberately one of them: an id now lives only
/// under `/failures/<id>`.
#[test]
fn a_path_this_does_not_serve_is_refused() {
    for line in [
        "GET /47 HTTP/1.1\r\n",
        "GET /failures/47/ack HTTP/1.1\r\n",
        "GET /failures/ HTTP/1.1\r\n",
        "GET /../secrets HTTP/1.1\r\n",
        "GET /?id=47 HTTP/1.1\r\n",
        "GET  HTTP/1.1\r\n",
    ] {
        assert_eq!(target(line), None, "{line:?} was served");
    }
}

/// READ-ONLY IS A PROPERTY OF THE PARSER, not of what the handlers happen to
/// do. A page reachable from a phone over a tunnel is not where an irreversible
/// action belongs, and refusing every other method is how that stays true when
/// a later handler is added.
#[test]
fn only_get_is_served() {
    for method in ["POST", "PUT", "DELETE", "HEAD", "OPTIONS"] {
        assert_eq!(
            target(&format!("{method} / HTTP/1.1\r\n")),
            None,
            "{method} was served"
        );
    }
}

/// A route name and a producer command are producer text, and this is the one
/// surface where producer text could become markup.
#[test]
fn producer_text_cannot_become_markup() {
    let rendered = html::sentence_page("route: <script>alert(1)</script> & more");
    assert!(!rendered.contains("<script>"), "{rendered}");
    assert!(rendered.contains("&lt;script&gt;"), "{rendered}");
    assert!(rendered.contains("&amp; more"), "{rendered}");
}

/// The escape runs ONCE over the record. An ampersand written by a producer
/// must not come back as `&amp;amp;`, which is the classic double-escape.
#[test]
fn an_ampersand_is_escaped_once() {
    assert_eq!(html::escaped("a & b"), "a &amp; b");
    assert_eq!(html::escaped("&lt;"), "&amp;lt;");
}

/// A double quote is escaped too: a producer string can land inside an
/// attribute value (`aria-label`), where an unescaped one would close it
/// early.
#[test]
fn a_double_quote_is_escaped() {
    assert_eq!(html::escaped("say \"hi\""), "say &quot;hi&quot;");
}

/// A bare sentence still lands inside a `<pre>`, so it reads as fixed-width
/// prose rather than being run into the shell's own markup.
#[test]
fn a_sentence_is_served_preformatted() {
    let rendered = html::sentence_page("pns: no failure 404");
    assert!(rendered.contains("<pre>"), "{rendered}");
    assert!(rendered.contains("pns: no failure 404"), "{rendered}");
}

/// `Content-Length` is what lets a phone's browser finish the response, and it
/// counts BYTES: an escaped record is longer than the text that went in.
#[test]
fn the_response_declares_the_length_of_the_body_it_carries() {
    let served = ok("a < b\n");
    let (head, body) = served.split_once("\r\n\r\n").expect("a header and a body");
    assert!(
        head.contains(&format!("Content-Length: {}", body.len())),
        "{head}"
    );
}

/// `Content-Length` COUNTS BYTES, not characters: every real page carries a
/// multi-byte character (the headline's `’`, the footer's `·`), and a phone
/// reading `chars().count()` bytes short would truncate the response.
#[test]
fn the_content_length_counts_bytes_not_chars() {
    let served = ok("’·"); // U+2019 (3 bytes) + U+00B7 (2 bytes): 2 chars, 5 bytes.
    let (head, body) = served.split_once("\r\n\r\n").expect("a header and a body");
    assert_eq!(
        body.chars().count(),
        2,
        "the fixture itself is two characters"
    );
    assert!(head.contains("Content-Length: 5"), "{head}");
}

/// The probe `moshi-hook` runs remembers listeners that answer with an HTTP
/// header, so a refusal answers with one too.
#[test]
fn a_refusal_is_still_an_http_response() {
    let refused = not_found();
    assert!(
        refused.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{refused}"
    );
    assert!(refused.contains("Content-Type: text/html"), "{refused}");
}

/// A sandbox store of this test's own, never the operator's real ledger: a
/// scratch path that does not end in the daemon's state suffix, so the
/// diagnostic log it opens lands beside it rather than in a real `$HOME`.
///
/// CANONICALIZED, because SQLite's `SQLITE_OPEN_NOFOLLOW` refuses to open a
/// database reached through a symlinked path component, and `/var` (which
/// `std::env::temp_dir()` resolves under on macOS) is one.
fn sandbox_store() -> SqliteStore {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("the canonical temp directory");
    SqliteStore::new(root.join(format!(
        "pns-failures-page-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    )))
}

/// THE SOCKET HALF, over a real connection on an OS-assigned port. Everything
/// above tests a string; this is the only thing that proves a request is read
/// off the wire and a response written back to it.
#[test]
fn a_real_request_gets_a_real_response() {
    use std::io::Read;
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    let store = sandbox_store();
    std::thread::spawn(move || serve_on_within(listener, &store, REQUEST_TIMEOUT));

    let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("a connection");
    client
        .write_all(b"GET /nowhere HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .expect("the request");
    let mut answered = String::new();
    client.read_to_string(&mut answered).expect("the response");

    assert!(
        answered.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{answered}"
    );
    assert!(answered.contains("<pre>"), "{answered}");
}

/// THE WHOLE SITE, over one real connection each: the index at `/`, the
/// listing at `/failures`, and the old bare `/<id>` refused now that an id
/// lives only under `/failures/<id>`.
#[test]
fn the_index_the_listing_and_the_retired_bare_id_route_all_answer_correctly() {
    use std::io::Read;
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    let store = sandbox_store();
    store
        .drain_deadlettered_legs()
        .expect("an empty write primes the schema");
    std::thread::spawn(move || serve_on_within(listener, &store, REQUEST_TIMEOUT));

    let get = |path: &str| -> String {
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("a connection");
        client
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .expect("the request");
        let mut answered = String::new();
        client.read_to_string(&mut answered).expect("the response");
        answered
    };

    let index = get("/");
    assert!(index.starts_with("HTTP/1.1 200 OK\r\n"), "{index}");
    assert!(index.contains(">none<"), "{index}");
    assert!(index.contains("href=\"/failures\">Failures</a>"), "{index}");

    let listing = get("/failures");
    assert!(listing.starts_with("HTTP/1.1 200 OK\r\n"), "{listing}");
    assert!(
        listing.contains("Nothing is failing to deliver."),
        "{listing}"
    );

    let retired = get("/47");
    assert!(
        retired.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{retired}"
    );
}

/// A CLIENT THAT NEVER SENDS A LINE MUST NOT WEDGE THE LOOP. The server is
/// single threaded, so without a read timeout the first, idle connection would
/// block `answer` forever and every request behind it would hang too.
#[test]
fn a_stalled_connection_does_not_block_the_next_request() {
    use std::io::Read;
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    let store = sandbox_store();
    std::thread::spawn(move || {
        serve_on_within(listener, &store, std::time::Duration::from_millis(50))
    });

    // CONNECTS FIRST AND SENDS NOTHING, so it is the connection the loop is
    // stuck answering when the second one arrives.
    let stalled = std::net::TcpStream::connect(("127.0.0.1", port)).expect("a connection");

    let mut second =
        std::net::TcpStream::connect(("127.0.0.1", port)).expect("a second connection");
    second
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .expect("the request");
    let mut answered = String::new();
    second.read_to_string(&mut answered).expect("the response");

    assert!(
        answered.starts_with("HTTP/1.1 200 OK\r\n"),
        "the second request was never answered: {answered:?}"
    );
    drop(stalled);
}

/// THE LINE IS RATE LIMITED. A copy per retry wrote sixteen thousand of one
/// sentence into the daemon's log; the repeat that survives is what keeps a
/// standing refusal readable after a rotation.
#[test]
fn a_standing_bind_refusal_is_said_once_and_then_every_ten_minutes() {
    let first = std::time::Instant::now();
    assert!(say_now(None, first), "the first refusal is always said");
    assert!(
        !say_now(Some(first), first + REBIND_AFTER),
        "the next retry repeats it"
    );
    assert!(
        !say_now(Some(first), first + RESAY_AFTER - REBIND_AFTER),
        "a retry inside the window repeats it"
    );
    assert!(
        say_now(Some(first), first + RESAY_AFTER),
        "a refusal still standing ten minutes on is said again"
    );
}

/// An empty ledger, an unreadable one, and an id nothing names: the answers
/// `index`, `listing` and `one` give when there is nothing to lay out, each
/// the exact sentence the terminal prints for the same case.
#[test]
fn empty_unreadable_and_unknown_answer_with_the_terminals_own_sentences() {
    let empty = sandbox_store();
    // A store with no schema on disk yet reads the same as an unreadable one,
    // so the empty case is primed with a write first, the way the daemon's
    // first submission would have created the database before any listing
    // ever ran against it.
    empty
        .drain_deadlettered_legs()
        .expect("an empty write primes the schema");
    assert!(index(&empty, 0).contains(">none<"));
    assert!(listing(&empty, 0).contains("Nothing is failing to deliver."));
    assert!(
        one(&empty, 404, 0)
            .contains("pns: no failure 404; run `pns failures` for the current list")
    );

    // A path that exists but is not a database: `read_only` fails to open it,
    // which is what an unreadable ledger looks like from here.
    let broken = std::env::temp_dir()
        .canonicalize()
        .expect("the canonical temp directory")
        .join(format!(
            "pns-failures-page-test-broken-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
    std::fs::create_dir_all(&broken).expect("the scratch directory");
    std::fs::write(broken.join("pns.db"), b"not a sqlite file").expect("the garbage file");
    let unreadable = SqliteStore::new(broken);
    assert!(index(&unreadable, 0).contains("pns: the delivery ledger could not be read"));
    assert!(listing(&unreadable, 0).contains("pns: the delivery ledger could not be read"));
    assert!(one(&unreadable, 1, 0).contains("pns: the delivery ledger could not be read"));
}

/// The 404 body names all three routes this page serves, escaped the way
/// every sentence inside the shell's `<pre>` is.
#[test]
fn the_404_names_the_three_routes_this_page_serves() {
    assert!(
        not_found().contains("pns: this page serves /, /failures and /failures/&lt;id&gt;"),
        "{}",
        not_found()
    );
}
