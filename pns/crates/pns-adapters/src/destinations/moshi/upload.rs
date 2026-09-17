//! The upload leg: the image bytes to moshi's own image host, so the webhook
//! has a public address to put on the card.
//!
//! THE TOKEN RIDES A HEADER HERE, AND ONLY HERE. Measured against the live
//! endpoint on 2026-09-15 with bogus tokens: this route reads the token from
//! an `Authorization: Bearer` header and nowhere else (a bogus header answers
//! 401 "Invalid token"; the token in the body with no header answers 401
//! "Missing or invalid Authorization header"), while the webhook beside it
//! requires the token in the BODY and answers a header-only request 422
//! naming property `/token`. A header is in-process, is not argv and is not a
//! child's environment, which is the rule the module above actually keeps.
//!
//! ONE FORM FIELD AND A FIXED BOUNDARY, rather than a multipart crate: the
//! only thing pns ever uploads is one PNG it drew itself.

/// Where the bytes go when `PNS_MOSHI_UPLOAD_URL` says nothing.
pub const DEFAULT_MOSHI_UPLOAD_URL: &str = "https://api.getmoshi.app/api/v1/images/upload";

/// What separates the one part of the upload body.
///
/// FIXED RATHER THAN GENERATED, and `multipart` REFUSES a payload that spells
/// it rather than producing a body the endpoint would mis-split. A PNG of
/// text cannot contain this run of bytes, so the refusal is a guard and not a
/// path anything takes.
const BOUNDARY: &str = "pns-card-image-4f8a1d0c6b27";

/// The `Content-Type` the body below has to be sent as.
pub(super) fn content_type() -> String {
    format!("multipart/form-data; boundary={BOUNDARY}")
}

/// The upload body: one `file` part carrying the PNG, or `None` when the
/// bytes could be mistaken for the boundary.
pub(crate) fn multipart(png: &[u8]) -> Option<Vec<u8>> {
    let marker = format!("--{BOUNDARY}").into_bytes();
    if png.windows(marker.len()).any(|window| window == marker) {
        return None;
    }
    let mut body = format!(
        "--{BOUNDARY}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"card.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Some(body)
}

/// The code the upload answered with, or `None` for a reply this cannot vouch
/// for.
///
/// THE CODE BECOMES A URL PATH SEGMENT, so it is checked rather than trusted:
/// the documented shape is eight alphanumeric characters, and anything
/// carrying a slash, a query or a space would build an address pointing
/// somewhere else. A refused code falls back to the text card.
pub(super) fn uploaded_code(reply: &str) -> Option<String> {
    let wire: serde_json::Value = serde_json::from_str(reply).ok()?;
    let code = wire["code"].as_str()?;
    (!code.is_empty()
        && code.len() <= MAX_CODE
        && code
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then(|| code.to_string())
}

/// The longest code this accepts. The documented one is eight characters;
/// this is room for the shape to grow without being room for a path.
const MAX_CODE: usize = 64;

/// The public address of an uploaded image, which is what the card carries.
///
/// UNAUTHENTICATED BY DESIGN: moshi hands the URL to Expo, which fetches it
/// server-side, so the address has to be reachable without the token. The
/// link expires after a day, which is longer than any card is worth reading.
pub(super) fn image_url(code: &str) -> String {
    format!("https://i.getmoshi.app/{code}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_carries_one_file_part_with_the_bytes_between_the_boundaries() {
        let body = multipart(b"PNG-ish").expect("ordinary bytes upload");
        let text = String::from_utf8(body).expect("the fixture is text-safe");
        assert!(text.starts_with(&format!("--{BOUNDARY}\r\n")));
        assert!(text.contains("name=\"file\"; filename=\"card.png\""));
        assert!(text.contains("Content-Type: image/png\r\n\r\nPNG-ish\r\n"));
        assert!(text.ends_with(&format!("--{BOUNDARY}--\r\n")));
        assert_eq!(
            content_type(),
            format!("multipart/form-data; boundary={BOUNDARY}")
        );
    }

    #[test]
    fn a_payload_that_spells_the_boundary_is_refused_rather_than_mis_split() {
        let spelled = format!("before--{BOUNDARY}after").into_bytes();
        assert!(multipart(&spelled).is_none());
    }

    #[test]
    fn the_code_is_read_off_the_documented_reply() {
        let reply = r#"{"id":"abcde","code":"abcde1xy","expires_at":"2026-07-23T12:00:00.000Z"}"#;
        assert_eq!(uploaded_code(reply).as_deref(), Some("abcde1xy"));
        assert_eq!(image_url("abcde1xy"), "https://i.getmoshi.app/abcde1xy");
    }

    #[test]
    fn a_code_that_would_not_stand_as_a_path_segment_is_refused() {
        // THE MUTANT THIS PINS: the code taken as given, which lets the
        // endpoint's reply choose the address the card points at.
        for code in ["", "../secret", "a b", "a/b", "a?b", "a.b", &"x".repeat(65)] {
            let reply = serde_json::json!({ "code": code }).to_string();
            assert_eq!(uploaded_code(&reply), None, "`{code}` was accepted");
        }
        assert_eq!(uploaded_code("not json"), None);
        assert_eq!(
            uploaded_code(r#"{"id":"abcde"}"#),
            None,
            "a reply with no code"
        );
    }
}
