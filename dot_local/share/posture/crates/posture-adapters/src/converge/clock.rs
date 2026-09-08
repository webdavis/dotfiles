use posture_application::RestartClock;
use std::time::{Duration, Instant};

pub struct RestartTimer(Instant);
impl Default for RestartTimer {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl RestartClock for RestartTimer {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
