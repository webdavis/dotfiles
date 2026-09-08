#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanaryEpoch(u64);
impl CanaryEpoch {
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty()
            || text.len() > 10
            || !text.bytes().all(|byte| byte.is_ascii_digit())
            || (text.len() > 1 && text.starts_with('0'))
        {
            return None;
        }
        text.parse().ok().map(Self)
    }
    pub fn seconds(self) -> u64 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanaryFreshness {
    Fresh { age: u64 },
    Missing,
    Stale { age: u64 },
    Implausible { skew: u64 },
}
pub fn canary_freshness(
    now: u64,
    canary: Option<CanaryEpoch>,
    maximum_age: u64,
) -> CanaryFreshness {
    let Some(canary) = canary else {
        return CanaryFreshness::Missing;
    };
    let timestamp = canary.seconds();
    if now.abs_diff(timestamp) <= maximum_age {
        CanaryFreshness::Fresh {
            age: now.saturating_sub(timestamp),
        }
    } else if timestamp > now {
        CanaryFreshness::Implausible {
            skew: timestamp - now,
        }
    } else {
        CanaryFreshness::Stale {
            age: now - timestamp,
        }
    }
}
#[cfg(test)]
mod tests;
