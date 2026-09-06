//! PNS policy, expressed as total functions of their arguments.
//!
//! This crate is responsible for what PNS DECIDES: the producer-neutral
//! notification and event types, the normalized signal, the delivery plan,
//! surface, presence and visibility arbitration, quiet and dim windows, pulse
//! and lighting precedence, home-probe identity and staleness, missed
//! notification replay, recap timeline and budget, nag cadence, job
//! scheduling, and the value types that make an invalid combination
//! unrepresentable.
//!
//! It is responsible for none of what PNS TOUCHES. No filesystem, SQLite,
//! TOML, JSON, HTTP, environment variable, spawned process, macOS API, Hue or
//! UniFi call, channel discovery or command-line output appears here or in
//! anything it depends on.
//!
//! The policy arrives one behavior at a time from the legacy package at the
//! workspace root, and each move is verified against the recorded test-name
//! set in `docs/test-baseline.md`. The root package re-exports what has landed
//! here, so every existing caller keeps its old path until the composition
//! step removes the re-exports.

pub mod count;
pub mod lamps;
pub mod lights;
pub mod missed;
pub mod pulse;
pub mod quiet;
pub mod recap;
pub mod registry;
pub mod render;
pub mod routing;
pub mod safety;
pub mod surface;
