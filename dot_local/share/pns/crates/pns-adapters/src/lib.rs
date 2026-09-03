//! The concrete edges: one module per real capability, never one broad
//! infrastructure module.
//!
//! This crate is responsible for implementing the ports pns-application
//! declares, against the actual things they stand for: the notification
//! destinations, the Hue attention indicator, the UniFi home-probe source, the
//! macOS idle and lock readers, the mosh terminal reader, the herdr view
//! reader, the filesystem protocols that remain protocols, SQLite persistence,
//! configuration loading and rendering, bounded process execution, the recap
//! sources and the summary providers.
//!
//! It is responsible for no policy. An adapter reports what it observed or
//! what a delivery did; what that means is decided in pns-domain, and which
//! order it happens in is decided in pns-application.
//!
//! Nothing has moved in yet.
