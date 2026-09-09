#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryLimits {
    pub max_attempts: u64,
    pub max_age_secs: u64,
}
impl Default for RetryLimits {
    fn default() -> Self {
        Self {
            max_attempts: 20,
            max_age_secs: 604800,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadletterReason {
    Attempts,
    Age,
}
impl RetryLimits {
    pub fn exhausted(self, retries: u64, created: u64, now: u64) -> Option<DeadletterReason> {
        if retries >= self.max_attempts {
            Some(DeadletterReason::Attempts)
        } else if created > 0 && now.saturating_sub(created) > self.max_age_secs {
            Some(DeadletterReason::Age)
        } else {
            None
        }
    }
}
