//! The fingerprint a TLS peer is pinned to, as a value.
//!
//! PURE, AND IT HASHES NOTHING. Parsing and comparing a pin is policy, so the
//! config layer can refuse a malformed one before any transport exists; taking
//! the digest of a presented certificate is the transport's job.
//!
//! COMPARISON IS OVER THE 32 RAW BYTES, never over the text, so case and
//! whitespace cannot make two pins differ.

/// The algorithm prefix, and the only one. A future algorithm is a NEW prefix
/// rather than a length check, so two pins can never be ambiguous about which
/// digest they name.
const SHA256_PREFIX: &str = "sha256:";

/// The SHA-256 digest a peer's certificate must equal, exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificatePin([u8; 32]);

impl CertificatePin {
    /// The pin the wire form names, or the refusal that quotes what was
    /// written.
    pub fn parse(text: &str) -> Result<Self, String> {
        let Some(hex) = text.strip_prefix(SHA256_PREFIX) else {
            return Err(format!(
                "{text:?} does not begin with `{SHA256_PREFIX}`, which is the only \
                 digest a pin may name"
            ));
        };
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "{text:?} does not carry 64 hexadecimal characters after `{SHA256_PREFIX}`"
            ));
        }
        let mut digest = [0u8; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            // Both halves are known hexadecimal by the check above, so this
            // cannot fail; the slice indices are in range for the same reason.
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap_or_default();
        }
        Ok(Self(digest))
    }

    /// The pin a digest already in hand stands for.
    pub fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Whether a presented digest is the pinned one.
    pub fn matches(&self, digest: &[u8]) -> bool {
        digest == self.0
    }
}

/// The wire form back, so an enrollment's output and a config's value are
/// literally the same string.
impl std::fmt::Display for CertificatePin {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(SHA256_PREFIX)?;
        for byte in self.0 {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIRE: &str = "sha256:90ab633b3a5607129abe4fa4c94811ee0f0d05c5ad758837890f0fd611621200";

    #[test]
    fn a_pin_parses_from_its_wire_form_and_displays_back_to_it() {
        let pin = CertificatePin::parse(WIRE).expect("a well formed pin");
        assert_eq!(pin.to_string(), WIRE);
    }

    #[test]
    fn an_uppercase_pin_is_the_same_pin_as_its_lowercase_spelling() {
        assert_eq!(
            CertificatePin::parse(&WIRE.to_uppercase().replace("SHA256:", "sha256:")),
            CertificatePin::parse(WIRE)
        );
    }

    #[test]
    fn a_pin_matches_the_digest_it_names_and_nothing_else() {
        let pin = CertificatePin::parse(WIRE).expect("a well formed pin");
        let mut digest = [0u8; 32];
        digest[0] = 0x90;
        digest[1] = 0xab;
        assert!(!pin.matches(&digest));
        let same = CertificatePin::parse(&pin.to_string()).expect("its own wire form");
        assert!(pin.matches(&same.0));
    }

    #[test]
    fn a_pin_that_is_not_sha256_plus_64_hex_characters_is_refused_quoting_it() {
        for offender in [
            "",
            "90ab633b",
            "sha1:90ab633b",
            "sha256:",
            "sha256:90ab",
            "sha256:zzab633b3a5607129abe4fa4c94811ee0f0d05c5ad758837890f0fd611621200",
        ] {
            let refusal = CertificatePin::parse(offender).expect_err("a refusal");
            assert!(refusal.contains(&format!("{offender:?}")), "{refusal}");
        }
    }
}
