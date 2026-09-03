//! The versioned, source-neutral wire contracts PNS speaks at its edges.
//!
//! This crate is responsible for two envelopes and their compatibility rules:
//! the producer request and result carried over `pns submit --json`, and the
//! egress envelope handed to an executable delivery destination. Each carries
//! a schema identifier with a major version, a request identifier, tagged
//! enums rather than boolean bags, an `extensions` object for producer
//! specific data, and the bounds enforced at the boundary (bytes, field count,
//! text length, collection length, nesting depth).
//!
//! It is responsible for no behavior behind those envelopes. It holds no
//! policy, no transport, no persistence, and no view of the domain model: an
//! external contract that imported the internal model would be dictated by it.
//!
//! Nothing has moved in yet. The envelopes are written test-first when the
//! protocol step reaches them, because they are new behavior rather than a
//! move.
