use std::time::{Duration, Instant};
use uu_application::{ClockFailure, RunClock};

pub struct SystemRunClock;

impl RunClock for SystemRunClock {
    type Tick = Instant;

    fn epoch(&self) -> Result<i64, ClockFailure> {
        crate::system::now_epoch().ok_or(ClockFailure::BeforeEpoch)
    }

    fn start(&self) -> Instant {
        Instant::now()
    }

    fn elapsed(&self, tick: &Instant) -> Duration {
        tick.elapsed()
    }
}
