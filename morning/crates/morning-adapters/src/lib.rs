//! Everything morning touches outside itself: its config file, the files it
//! reads, and the commands it spawns.

pub mod capture;
pub mod config;
pub mod files;

pub use capture::{CaptureError, capture};
pub use config::{CommandSource, Config, LedgerSource, PathSource, RecapSource};
