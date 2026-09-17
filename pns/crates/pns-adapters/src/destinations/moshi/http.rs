use super::HttpPost;

/// The deadline one moshi post runs under. Nobody waits on the answer and
/// nothing is retried, so this only bounds how long the process lingers.
pub const POST_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// The production POST: one agent, one deadline, no retry. Every failure is
/// `false` and nothing is logged, because the only thing worth reporting
/// would be the request that carries the token.
pub struct UreqPost {
    /// The whole-request deadline. Production uses the default; tests hand
    /// in a short one to prove the deadline actually fires.
    pub timeout: std::time::Duration,
}

impl Default for UreqPost {
    fn default() -> Self {
        Self {
            timeout: POST_DEADLINE,
        }
    }
}

impl HttpPost for UreqPost {
    fn post_json(&self, url: &str, body: &str) -> bool {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // The bash curl carried no -L, and following one would send the
            // token to whatever host the endpoint names. Zero returns the 3xx
            // as the response rather than an error, so the post simply ends.
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .send(body)
            // NOT `is_ok`. With no redirects followed, a 3xx comes back as a
            // RESPONSE rather than an error, so `is_ok` answered true for a
            // card the endpoint bounced somewhere else and never delivered.
            .is_ok_and(|response| DELIVERED_STATUS.contains(&response.status().as_u16()))
    }

    /// The multipart upload, under the same deadline and the same no-redirect
    /// rule as the post above, with the token in an `Authorization` header
    /// because that is the only placement this route accepts.
    fn upload_png(&self, url: &str, token: &str, png: &[u8]) -> Option<String> {
        let body = super::upload::multipart(png)?;
        let mut response = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // Following one would send the token to whatever host the
            // endpoint names, exactly as on the webhook beside it.
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .header("Authorization", &format!("Bearer {token}"))
            .content_type(super::upload::content_type())
            .send(&body)
            .ok()?;
        if !DELIVERED_STATUS.contains(&response.status().as_u16()) {
            return None;
        }
        let reply = response
            .body_mut()
            .with_config()
            .limit(MAX_REPLY_BYTES)
            .read_to_string()
            .ok()?;
        super::upload::uploaded_code(&reply)
    }
}

/// How much of the upload's reply is read. The documented reply is three
/// short fields; anything past this is not one.
const MAX_REPLY_BYTES: u64 = 4096;

/// The status codes that mean the card reached the phone. Spelled here rather
/// than shared with hermes: the two channels answer to different endpoints and
/// a range moved for one of them must not follow the other.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;
