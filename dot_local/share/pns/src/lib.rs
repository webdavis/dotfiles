//! pns: the decision core plus its thin edges.
//!
//! THE SPLIT THAT MATTERS. The decision modules (`surface`, `presence`,
//! `routing`, `render`, `pulse`, `safety`) are total functions of their
//! arguments: no
//! network, no files, no clock, no environment. That is what makes them
//! testable one behavior at a time, in microseconds, without stubbing a
//! subprocess. The edges (`system` reads the machine, `config` reads the
//! file) keep their IO one seam away from a pure parser, and the engine
//! binary will own the wiring.
//!
//! The decision modules never print, exit, or read the environment. A caller
//! decides what to do with a verdict, and the composition root is where
//! wiring lives.

pub mod args;
pub mod channels;
pub mod config;
pub mod config_text;
pub mod daemon;
pub mod decision_log;
pub mod doctor;
pub mod engine;
pub mod focus;
pub mod home;
pub mod hooks;
pub mod lights;
pub mod missed_notifications;
pub mod nag;
pub mod presence;
pub mod presence_file;
pub mod presence_hue;
pub mod presence_instant;
pub mod presence_journal;
pub mod presence_lock;
pub mod presence_policy;
pub mod presence_room;
pub mod probes;
pub mod pulse;
pub mod quiet;
pub mod recap;
pub mod registry;
pub mod routing;
pub mod setup;
pub mod surface;
pub mod system;

// WHAT HAS MOVED INTO `pns-domain`, re-exported so every caller keeps its old
// path. The re-exports go when the composition root does.
pub use pns_domain::count::parse_count;
pub use pns_domain::render;
pub use pns_domain::safety;
