//! One use case per entry point, over the ports it declares.
//!
//! This crate is responsible for ORDERING: each use case is a concrete struct
//! that performs the calls the corresponding bash `main` performs today, over
//! traits it owns for every external capability it consumes (a results log, a
//! cursor store, a single-instance lock, an allowlist, a known-good manifest,
//! a deployed-state reader, a spool, an alert sink, a clock, and the process
//! and privilege boundaries the converge needs).
//!
//! The one port every producer shares is the alert sink, whose accepted answer
//! means the engine has committed a retriable obligation for the request and
//! not merely that a channel was tried. Notify-before-persist is therefore one
//! line in each producer: state advances only on an accepted submission. The
//! watchdog owns separate ledger-health and independent-alarm ports; its pns
//! integrity/health alarm runs regardless of an engine acknowledgement.
//!
//! It is responsible for no concrete I/O and no policy: policy lives in
//! `posture-domain`, and every trait here is implemented from the outside by
//! `posture-adapters`. Nothing has moved in yet.
