//! `pns doctor`: what one test send through every configured channel found.
//!
//! POLICY ONLY, and every function here is a total function of its arguments:
//! no config, no clock, no environment, no network, no printing. The binary
//! reads the world, sends through the engine's own wiring, and hands what came
//! back to these to shape.
//!
//! THE CENSUS IS THE WHOLE ROSTER, never the selection. A plugin the config
//! left off has to be visibly absent BY CHOICE, or the report answers "what is
//! on" when the operator asked "what will reach me", which is the narrower
//! predicate this project keeps re-finding.

mod census;
mod daemon;
mod lights;
mod outcome;
mod pairing;
mod presence;
mod report;
mod routes;
pub use census::checks;
pub use daemon::{daemon_line, nag_line};
pub use lights::{LightsReport, lights_lines};
pub use outcome::{Check, CheckKind, ConfigState, Outcome, exit_code, line, outcome_mark, summary};
pub use pairing::{Pairing, PairingReport, pairing_lines, pairing_mark};
pub use report::{Item, Mark};
pub use routes::{RouteVerdict, route_line, route_mark, routes_summary};

mod decisions;
pub use decisions::section as decision_section;
