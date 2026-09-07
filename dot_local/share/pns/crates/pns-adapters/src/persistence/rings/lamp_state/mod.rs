use crate::publish_state_line;
use std::path::Path;
use std::time::Duration;
mod complaints;
mod held;
mod news;
pub use complaints::{LIGHTS_SAID, say_lights_once};
pub use held::{LIGHTS_HELD, held_lamps, read_held, remember_held};
pub use news::{read_news, record_news};

mod streak;
pub use streak::advance_streak;
mod muted;
pub use muted::{LIGHTS_QUIET, LIGHTS_QUIET_SAID, muted_state, publish_muted};

#[cfg(test)]
mod tests;
