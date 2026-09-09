use crate::{Drift, LiveAttributes};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTrustRefusal {
    Relative,
    Unreadable,
    Owner(u32),
    Writable(u32),
}

pub fn command_trust(
    absolute: bool,
    attributes: Option<LiveAttributes>,
) -> Result<(), CommandTrustRefusal> {
    if !absolute {
        return Err(CommandTrustRefusal::Relative);
    }
    let attributes = attributes.ok_or(CommandTrustRefusal::Unreadable)?;
    if attributes.uid != 0 {
        return Err(CommandTrustRefusal::Owner(attributes.uid));
    }
    if attributes.mode & 0o022 != 0 {
        return Err(CommandTrustRefusal::Writable(attributes.mode));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentPid(u64);
impl ParentPid {
    pub fn parse(text: &str) -> Option<Self> {
        decimal(text, 10).map(Self)
    }
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartBounds {
    deadline: Duration,
    settle: Duration,
}
impl RestartBounds {
    pub fn parse(deadline: Option<&str>, settle: Option<&str>) -> Self {
        Self {
            deadline: Duration::from_secs(deadline.and_then(|text| decimal(text, 4)).unwrap_or(30)),
            settle: Duration::from_secs(settle.and_then(|text| decimal(text, 4)).unwrap_or(5)),
        }
    }
    pub fn deadline(self) -> Duration {
        self.deadline
    }
    pub fn settle(self) -> Duration {
        self.settle
    }
    pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
}

impl Drift {
    pub fn label(self) -> &'static str {
        match self {
            Self::Absent => "missing",
            Self::Irregular => "not a regular file",
            Self::Unreadable => "unreadable",
            Self::Content => "content drift",
            Self::Mode => "mode drift",
            Self::Owner => "owner drift",
            Self::Group => "group drift",
            Self::Ok => "drift",
        }
    }
}

fn decimal(text: &str, max_digits: usize) -> Option<u64> {
    if text.is_empty()
        || text.len() > max_digits
        || text.starts_with('0')
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests;
