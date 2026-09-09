//! The digest record shared by the alert writer and daily digest reader.
//!
//! PR 2.4 implements the existing six-field record codec and compatibility
//! tests. Its field names, coercions, torn-line handling and unversioned wire
//! shape stay unchanged. This foundation declares the responsibility only.
//!
//! File append, claim, restore and permissions belong to adapters. Derivation,
//! grouping, sanitization and caps belong to domain policy. Notification
//! request/result envelopes remain owned by the sibling pns-protocol crate;
//! this crate neither copies nor forwards them.
