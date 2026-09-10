//! The versioned wire contracts posture speaks to the pns engine.
//!
//! POSTURE'S OWN COPY OF A CONTRACT IT DOES NOT OWN. pns defines these
//! envelopes; posture is a producer that submits requests to `pns submit
//! --json` and reads the result it gets back. The two ship as separate
//! projects, so neither can compile against the other, and each carries its own
//! reading of the same bytes. The fixtures beside this crate are what hold the
//! two readings together: they are the same golden documents pns pins, so a
//! change on either side that moves the bytes fails a test rather than a
//! delivery.
//!
//! The request and result envelopes define the `pns submit --json` boundary.
//! Each carries a schema identifier with a major version, a request identifier
//! and typed outcomes or signals. Requests retain producer-specific data under
//! `extensions`. Both envelopes enforce the same byte, field, text, collection
//! and nesting limits.
//!
//! The egress envelope pns also defines is absent here. It carries a rendered
//! event from pns out to a delivery destination, which posture is not.
//!
//! It is responsible for no behavior behind those envelopes. It holds no
//! policy, no transport, no persistence, and no view of the domain model: an
//! external contract that imported the internal model would be dictated by it.
//!
//! The compatibility policy, in one place:
//!
//! - The schema identifier is `<name>/<major>`. A major that the crate does
//!   not know is refused clearly ([`Rejection::MajorUnsupported`]).
//!   Additive change within a major does not bump it.
//! - Unknown fields in a known major are ignored within the wire bounds.
//!   Unknown request top-level fields are also named
//!   ([`DecodedRequest::ignored`]), so an older pns keeps working against a
//!   newer producer and the producer can still learn its field went nowhere.
//! - Producer-specific data goes under `extensions`, which is carried
//!   verbatim, bounded, and never interpreted here.
//! - Text is carried verbatim inside the caps. Sanitizing is the domain's
//!   job, where the destination it is bound for is known.
//! - Duplicate object fields are refused at every depth, including schema,
//!   request identifiers and extensions. No occurrence wins over another.
//! - Encoding enforces the same bounds as decoding. Result diagnostics keep
//!   their first 64 codes; other over-cap fields are refused. Diagnostic codes
//!   are advisory, while dropping destination outcomes would hide deliveries.

mod bounds;
mod envelope;
mod identifiers;
mod request;
mod result;

pub use bounds::{MAX_BYTES, MAX_DEPTH, MAX_FIELDS, MAX_ITEMS, MAX_TEXT_CHARS, Violation};
pub use envelope::{Rejected, Rejection};
pub use identifiers::{InvalidIdentifier, NAME_MAX_CHARS, Name, REQUEST_ID_MAX_CHARS, RequestId};
pub use request::{
    Context, Decoded as DecodedRequest, DeliveryScope, Interaction, Request, Session, Signal,
    decode as decode_request,
};
pub use result::{
    DeliveryOutcome, DestinationOutcome, InteractionResult, ResultEnvelope, Status,
    decode as decode_result,
};
