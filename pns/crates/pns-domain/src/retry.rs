//! How long a failed delivery leg keeps trying, and when it stops for good.
//!
//! THE PERMANENT-VERSUS-TEMPORARY SPLIT LIVES HERE, once, so the retry loop and
//! the failure reporting read the same rule. It applies to EVERY destination:
//! a refusal will not fix itself whatever answered it, so moshi and any later
//! HTTP destination classify the same way. What varies per destination is the
//! wording shown to the operator, never the classification.

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

/// What one delivery attempt came back with, in domain terms. The transport
/// crates carry their own outcome types; they map onto this so the rule below
/// stays in a crate with no dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Status(u16),
    /// The request went out and nothing answered.
    NoResponse,
    /// The request was never sendable, a malformed URL being the case that
    /// prompted this: no status, and no amount of waiting produces one.
    NoStatus,
}

/// Whether trying again could ever help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// The request was understood and refused, or was never sendable.
    Permanent,
    /// The destination may recover.
    Temporary,
}
impl FailureClass {
    pub fn is_permanent(self) -> bool {
        matches!(self, FailureClass::Permanent)
    }
}

impl DeliveryOutcome {
    /// Whether this outcome is a delivery at all. 2xx and nothing else: a
    /// redirect was never followed, so it delivered nothing.
    pub fn delivered(self) -> bool {
        matches!(self, DeliveryOutcome::Status(status) if (200..300).contains(&status))
    }

    /// The class of this outcome READ AS A FAILURE, whether or not it is one.
    /// A caller that already knows delivery failed uses this directly; a caller
    /// holding an unclassified outcome uses [`Self::failure_class`], which
    /// answers None for a success.
    ///
    /// Total by construction. The listed codes come from the design's tables;
    /// an unlisted code falls to its family, because a 5xx is the destination
    /// claiming the fault is its own while everything else is it saying the
    /// request was wrong. An unrecognized status outside both families cannot
    /// be retried into working either, so it is permanent.
    pub fn class(self) -> FailureClass {
        let status = match self {
            // Nothing answered, so the destination may simply be down.
            DeliveryOutcome::NoResponse => return FailureClass::Temporary,
            // Never reached the wire.
            DeliveryOutcome::NoStatus => return FailureClass::Permanent,
            DeliveryOutcome::Status(status) => status,
        };
        match status {
            408 | 429 => FailureClass::Temporary,
            500..=599 => FailureClass::Temporary,
            _ => FailureClass::Permanent,
        }
    }

    /// The failure class, or None when the outcome delivered.
    pub fn failure_class(self) -> Option<FailureClass> {
        (!self.delivered()).then(|| self.class())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadletterReason {
    Attempts,
    Age,
    /// The destination refused in a way repeating cannot fix. Dead-letters on
    /// the FIRST failure rather than the twentieth, which is the difference
    /// between a typo'd route costing one attempt and costing a week.
    Permanent,
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

    /// Whether this leg is finished, and why. Permanence is checked FIRST and
    /// outranks both counters, so a leg that crossed a limit in the same moment
    /// it was permanently refused records what actually stopped it.
    pub fn verdict(
        self,
        outcome: DeliveryOutcome,
        retries: u64,
        created: u64,
        now: u64,
    ) -> Option<DeadletterReason> {
        match outcome.failure_class()? {
            FailureClass::Permanent => Some(DeadletterReason::Permanent),
            FailureClass::Temporary => self.exhausted(retries, created, now),
        }
    }
}

/// When the next attempt is due. LINEAR AND DETERMINISTIC: the delay grows by
/// `base_secs` per attempt and nothing is added on top. An earlier draft
/// carried a random spread, which exists to stop many clients retrying in the
/// same instant; this is one local daemon draining one queue against a loopback
/// gateway, so there was no herd to spread and the randomness only made the
/// schedule untestable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBackoff {
    pub base_secs: u64,
}
impl Default for RetryBackoff {
    fn default() -> Self {
        Self { base_secs: 60 }
    }
}
impl RetryBackoff {
    pub fn retry_at(self, now: u64, retries: u64) -> u64 {
        now.saturating_add(self.base_secs.saturating_mul(retries))
    }
}

#[cfg(test)]
mod tests;
