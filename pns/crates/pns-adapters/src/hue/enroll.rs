//! `pns lights enroll`: read the certificate the bridge presents, and print the
//! line to save.
//!
//! IT PRINTS AND NEVER WRITES. The deployed config is a chezmoi target, so a
//! command that edited it would be reverted by the next apply; the value belongs
//! in the vault entry the values file already names.
//!
//! THE CAPTURE VERIFIER IS ITS OWN TYPE, not the pinned one with a flag. A
//! verifier that can be told to accept anything is one boolean away from
//! turning the whole feature off.

use pns_domain::CertificatePin;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{Connector, TcpConnector};

/// What one enrollment learned.
#[derive(Debug)]
pub struct Enrollment {
    /// The certificate's common name, which on a genuine bridge is its id.
    pub common_name: String,
    /// The pin the certificate hashes to.
    pub pin: CertificatePin,
    /// `bridgeid` from the unauthenticated `/api/config`, or None when the
    /// bridge did not answer that path.
    pub reported_id: Option<String>,
    /// `modelid` from the same answer.
    pub model: Option<String>,
}

impl Enrollment {
    /// Whether the certificate and the bridge's own answer name one device.
    pub fn identities_agree(&self) -> bool {
        self.reported_id
            .as_deref()
            .is_some_and(|reported| reported.eq_ignore_ascii_case(&self.common_name))
    }
}

/// Read the bridge's certificate and its self-reported identity.
///
/// TWO READINGS OF ONE HOST, and the whole point is that they can disagree.
pub fn enroll(bridge: &str, deadline: Duration) -> Result<Enrollment, String> {
    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let agent = ureq::Agent::with_parts(
        ureq::Agent::config_builder()
            .timeout_global(Some(deadline))
            .max_redirects(0)
            .http_status_as_error(false)
            .build(),
        TcpConnector::default().chain(super::pinned_tls::CapturingTlsConnector {
            captured: captured.clone(),
        }),
        DefaultResolver::default(),
    );
    let body = agent
        .get(format!("https://{bridge}/api/config"))
        .call()
        .map_err(|why| format!("pns: the bridge at {bridge} did not answer /api/config ({why})"))?
        .body_mut()
        .read_to_string()
        .unwrap_or_default();
    let Some(certificate) = captured.lock().ok().and_then(|held| held.clone()) else {
        return Err(format!(
            "pns: the bridge at {bridge} presented no certificate to enroll"
        ));
    };
    let reported = serde_json::from_str::<serde_json::Value>(&body).ok();
    let field = |name: &str| -> Option<String> {
        reported
            .as_ref()?
            .get(name)?
            .as_str()
            .map(str::to_string)
            .filter(|value| !value.is_empty())
    };
    Ok(Enrollment {
        common_name: common_name(&certificate)
            .ok_or_else(|| format!("pns: the certificate at {bridge} carries no common name"))?,
        pin: CertificatePin::from_digest(super::pinned_tls::certificate_digest(&certificate)),
        reported_id: field("bridgeid"),
        model: field("modelid"),
    })
}

/// The common name out of a DER-encoded certificate's subject.
///
/// A SCAN RATHER THAN A PARSER, and deliberately: the only thing wanted out of
/// this certificate is one printable string, and pulling in an X.509 parser to
/// get it would put a whole new attack surface in a non-test build for one
/// field. The attribute is found by its object identifier (2.5.4.3, encoded as
/// `55 04 03`) followed by a printable or UTF-8 string header.
fn common_name(certificate: &[u8]) -> Option<String> {
    const COMMON_NAME_OID: [u8; 5] = [0x06, 0x03, 0x55, 0x04, 0x03];
    let mut at = 0;
    while at + COMMON_NAME_OID.len() + 2 <= certificate.len() {
        if certificate[at..at + COMMON_NAME_OID.len()] != COMMON_NAME_OID {
            at += 1;
            continue;
        }
        let header = at + COMMON_NAME_OID.len();
        // PrintableString (0x13) or UTF8String (0x0c), short form length only:
        // a common name is a handful of characters and a long-form length here
        // would mean something other than a name.
        let tag = certificate[header];
        let length = certificate[header + 1] as usize;
        if !matches!(tag, 0x13 | 0x0c) || header + 2 + length > certificate.len() {
            at += 1;
            continue;
        }
        return String::from_utf8(certificate[header + 2..header + 2 + length].to_vec()).ok();
    }
    None
}

/// The capture-only verifier's own connector. It lives beside the pinned one
/// because it is the same 30-line transport copy; what differs is the verifier.
#[derive(Debug)]
pub(super) struct CaptureVerifier {
    pub(super) captured: Arc<Mutex<Option<Vec<u8>>>>,
}

impl ServerCertVerifier for CaptureVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if let Ok(mut held) = self.captured.lock() {
            *held = Some(end_entity.to_vec());
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
#[path = "enroll/tests.rs"]
mod tests;
