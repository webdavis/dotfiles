use crate::ValueError;

/// The algorithm prefix, and the only one. A future digest is a NEW prefix
/// rather than a length check, so two pins can never be ambiguous about which
/// algorithm they name.
const SHA256_PREFIX: &str = "sha256:";

/// The SHA-256 digest a peer's certificate must equal, exactly.
///
/// PURE, AND IT HASHES NOTHING: parsing and comparing a pin is policy, so the
/// config layer refuses a malformed one before any transport exists, while
/// taking the digest of a presented certificate is the transport's job.
/// COMPARISON IS OVER THE 32 RAW BYTES, never the text, so case and whitespace
/// cannot make two pins differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificatePin([u8; 32]);

impl CertificatePin {
    /// The pin the wire form names, or a refusal that quotes nothing: a config
    /// message here travels to stderr beside a bridge key, and this crate's
    /// rule is that a config refusal never echoes what it read.
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let Some(hex) = text.trim().strip_prefix(SHA256_PREFIX) else {
            return Err(ValueError("a certificate pin must begin with sha256:"));
        };
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ValueError(
                "a certificate pin must carry 64 hexadecimal characters after sha256:",
            ));
        }
        let mut digest = [0u8; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            // Both halves are known hexadecimal by the check above, and the
            // slice indices are in range for the same reason.
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

    const WIRE: &str = "sha256:2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881";

    #[test]
    fn a_pin_parses_from_its_wire_form_and_displays_back_to_it() {
        assert_eq!(CertificatePin::parse(WIRE).unwrap().to_string(), WIRE);
    }
    #[test]
    fn an_uppercase_pin_is_the_same_pin_as_its_lowercase_spelling() {
        let shouted = format!(
            "sha256:{}",
            WIRE.trim_start_matches("sha256:").to_uppercase()
        );
        assert_eq!(CertificatePin::parse(&shouted), CertificatePin::parse(WIRE));
    }
    #[test]
    fn a_pin_matches_the_digest_it_names_and_nothing_else() {
        let pin = CertificatePin::parse(WIRE).unwrap();
        assert!(pin.matches(&CertificatePin::parse(WIRE).unwrap().0));
        assert!(!pin.matches(&[0u8; 32]));
        assert!(!pin.matches(&[0u8; 31]));
    }
    #[test]
    fn a_pin_that_is_not_sha256_plus_64_hex_characters_is_refused() {
        for offender in [
            "",
            "2d711642",
            "sha1:2d711642",
            "sha256:",
            "sha256:2d71",
            "sha256:zz711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881",
        ] {
            assert!(CertificatePin::parse(offender).is_err(), "{offender}");
        }
    }
    #[test]
    fn a_refusal_never_echoes_the_value_it_read() {
        let refusal = CertificatePin::parse("sha256:secret-looking-nonsense").unwrap_err();
        assert!(!refusal.0.contains("secret"), "{}", refusal.0);
    }
}
