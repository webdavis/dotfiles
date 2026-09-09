use super::*;

/// The two routes the page serves, and nothing else needs a name.
#[test]
fn the_root_is_the_listing_and_a_number_is_one_record() {
    assert_eq!(target("GET / HTTP/1.1\r\n"), Some(Target::Listing));
    assert_eq!(target("GET /47 HTTP/1.1\r\n"), Some(Target::One(47)));
}

/// A page with two routes has nothing to say about a third, so a path it does
/// not know is refused rather than guessed at.
#[test]
fn a_path_this_does_not_serve_is_refused() {
    for line in [
        "GET /failures HTTP/1.1\r\n",
        "GET /47/ack HTTP/1.1\r\n",
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
    let rendered = page("route: <script>alert(1)</script> & more\n");
    assert!(!rendered.contains("<script>"), "{rendered}");
    assert!(rendered.contains("&lt;script&gt;"), "{rendered}");
    assert!(rendered.contains("&amp; more"), "{rendered}");
}

/// The escape runs ONCE over the record. An ampersand written by a producer
/// must not come back as `&amp;amp;`, which is the classic double-escape.
#[test]
fn an_ampersand_is_escaped_once() {
    assert_eq!(escaped("a & b"), "a &amp; b");
    assert_eq!(escaped("&lt;"), "&amp;lt;");
}

/// The record keeps its column alignment, which is the reason the full form is
/// laid out in columns at all.
#[test]
fn the_record_is_served_preformatted() {
    let rendered = page("  status:  HTTP 404\n");
    assert!(rendered.contains("<pre>"), "{rendered}");
    assert!(rendered.contains("  status:  HTTP 404"), "{rendered}");
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

/// THE SOCKET HALF, over a real connection on an OS-assigned port. Everything
/// above tests a string; this is the only thing that proves a request is read
/// off the wire and a response written back to it.
#[test]
fn a_real_request_gets_a_real_response() {
    use std::io::Read;
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    std::thread::spawn(move || serve_on(listener));

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
