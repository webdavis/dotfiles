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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBackoff {
    pub base_secs: u64,
    pub random_secs: u64,
}
impl Default for RetryBackoff {
    fn default() -> Self {
        Self {
            base_secs: 60,
            random_secs: 60,
        }
    }
}
impl RetryBackoff {
    pub fn retry_at(self, now: u64, retries: u64, sample: u16) -> u64 {
        // Preserve the legacy 15-bit sample and inclusive configured maximum.
        let offset = u64::from(sample & 0x7fff) % self.random_secs.saturating_add(1);
        now.saturating_add(self.base_secs.saturating_mul(retries))
            .saturating_add(offset)
    }
}

#[cfg(test)]
mod tests;
