//! The bridge's TLS, verified against one pinned certificate and nothing else.
//!
//! WHY THIS FILE EXISTS. The bridge's certificate carries no subjectAltName, so
//! every modern verifier refuses to match its name against the address it is
//! reached at, whatever trust anchor is configured. The only identity it
//! carries is its own fingerprint, so that is what is pinned. ureq exposes no
//! custom-verifier hook and couples native-tls's two danger flags into one, so
//! reaching a verifier means supplying a connector through `Agent::with_parts`
//! out of ureq's `unversioned::transport` items.
//!
//! THE HANDSHAKE COMPLETES INSIDE `connect`, which is what makes a pin mismatch
//! a refused connection rather than a mid-request surprise, and what lets the
//! record be taken before any request has been sent.

use lights_domain::CertificatePin;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};
use sha2::Digest;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, Transport,
    TransportAdapter,
};

/// The name sent and never verified. A FIXED PLACEHOLDER, because rustls needs
/// a `ServerName` to build a connection and the certificate carries none that
/// could ever match; server name indication is turned off below, so nothing
/// reaches the bridge.
const UNVERIFIED_NAME: &str = "hue-bridge.invalid";

/// The digest of one presented certificate, as the pin spells it.
pub fn certificate_digest(certificate: &[u8]) -> [u8; 32] {
    sha2::Sha256::digest(certificate).into()
}

/// A connector that accepts exactly one certificate.
#[derive(Debug)]
pub struct PinnedTlsConnector {
    pub pin: CertificatePin,
}
impl<In: Transport> Connector<In> for PinnedTlsConnector {
    type Out = Either<In, PinnedTlsTransport>;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = Arc::new(PinnedVerifier {
            pin: self.pin,
            provider: provider.clone(),
            address: authority(details),
        });
        connect_verified(details, chained, provider, verifier)
    }
}

/// A connector that accepts anything and keeps what it was shown. Enrollment's
/// own, and it exists to PRODUCE a pin rather than to check one.
#[derive(Debug)]
pub(super) struct CapturingTlsConnector {
    pub(super) captured: Arc<Mutex<Option<Vec<u8>>>>,
}
impl<In: Transport> Connector<In> for CapturingTlsConnector {
    type Out = Either<In, PinnedTlsTransport>;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = Arc::new(super::enroll::CaptureVerifier {
            captured: self.captured.clone(),
        });
        connect_verified(details, chained, provider, verifier)
    }
}

/// The one TLS connect path, whichever verifier decides.
fn connect_verified<In: Transport>(
    details: &ConnectionDetails,
    chained: Option<In>,
    provider: Arc<CryptoProvider>,
    verifier: Arc<dyn ServerCertVerifier>,
) -> Result<Option<Either<In, PinnedTlsTransport>>, ureq::Error> {
    let Some(transport) = chained else {
        return Err(ureq::Error::ConnectionFailed);
    };
    if !details.needs_tls() || transport.is_tls() {
        return Ok(Some(Either::A(transport)));
    }
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| ureq::Error::Tls("the pinned client has no usable TLS version"))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.enable_sni = false;
    let name = ServerName::try_from(UNVERIFIED_NAME)
        .map_err(|_| ureq::Error::Tls("the placeholder server name is not a name"))?;
    let mut stream = StreamOwned {
        conn: ClientConnection::new(Arc::new(config), name)?,
        sock: TransportAdapter::new(transport.boxed()),
    };
    stream.sock.set_timeout(details.timeout);
    // THE HANDSHAKE, HERE. A lazy one would surface a mismatch part way through
    // a request instead of as a refused connection.
    stream
        .conn
        .complete_io(&mut stream.sock)
        .map_err(ureq::Error::Io)?;
    Ok(Some(Either::B(PinnedTlsTransport {
        buffers: LazyBuffers::new(
            details.config.input_buffer_size(),
            details.config.output_buffer_size(),
        ),
        stream,
    })))
}

/// Where the connection was addressed, for the refusal to name.
fn authority(details: &ConnectionDetails) -> String {
    details
        .uri
        .authority()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// The verifier: one digest comparison, and rustls's own signature checks.
///
/// THE SIGNATURE CALLBACKS DELEGATE rather than asserting. Pinning answers
/// which certificate this is; it says nothing about whether the peer holds the
/// matching private key, and a verifier that waved the handshake's own
/// cryptography through would pin a certificate anyone could copy off the wire.
#[derive(Debug)]
struct PinnedVerifier {
    pin: CertificatePin,
    provider: Arc<CryptoProvider>,
    address: String,
}
impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let digest = certificate_digest(end_entity);
        if self.pin.matches(&digest) {
            return Ok(ServerCertVerified::assertion());
        }
        let presented = CertificatePin::from_digest(digest);
        super::mismatch::record(&self.address, self.pin, presented);
        Err(rustls::Error::General(super::mismatch::refusal(
            &self.address,
            self.pin,
            presented,
        )))
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// ureq's own rustls transport, copied because its type has private fields and
/// no constructor.
pub struct PinnedTlsTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}
impl std::fmt::Debug for PinnedTlsTransport {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("PinnedTlsTransport").finish()
    }
}
impl Transport for PinnedTlsTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.stream.sock.set_timeout(timeout);
        let output = &self.buffers.output()[..amount];
        self.stream.write_all(output)?;
        Ok(())
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.stream.sock.set_timeout(timeout);
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }
    fn is_open(&mut self) -> bool {
        self.stream.sock.get_mut().is_open()
    }
    fn is_tls(&self) -> bool {
        true
    }
}
