#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditBounds {
    pub entries: u32,
    pub bytes: u64,
    pub seconds: u64,
}

impl AuditBounds {
    pub fn from_values(entries: &str, bytes: &str, seconds: &str) -> Self {
        Self {
            entries: bounded(entries, 1, 100_000, 500) as u32,
            bytes: bounded(bytes, 1, 1_073_741_824, 8_388_608),
            seconds: bounded(seconds, 0, 300, 60),
        }
    }
}

fn bounded(value: &str, low: u64, high: u64, fallback: u64) -> u64 {
    if value.is_empty()
        || value.len() > 10
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return fallback;
    }
    value
        .parse()
        .ok()
        .filter(|number| (low..=high).contains(number))
        .unwrap_or(fallback)
}
