use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub struct SshReadiness {
    pub attempts: u32,
    pub interval: Duration,
    pub probe_timeout: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ReadinessRefusal {
    Attempts,
    Interval,
    ProbeTimeout,
    MissingPort,
    Port(Vec<u8>),
}
impl SshReadiness {
    pub fn parse(attempts: &str, interval: &str, timeout: &str) -> Result<Self, ReadinessRefusal> {
        let attempts = canonical_integer(attempts, 9).ok_or(ReadinessRefusal::Attempts)?;
        let probe_timeout = canonical_integer(timeout, 9).ok_or(ReadinessRefusal::ProbeTimeout)?;
        if interval.is_empty()
            || interval == "."
            || interval.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
            || interval.bytes().filter(|b| *b == b'.').count() > 1
            || (interval.starts_with('0')
                && interval.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
        {
            return Err(ReadinessRefusal::Interval);
        }
        let seconds: f64 = interval.parse().map_err(|_| ReadinessRefusal::Interval)?;
        let interval =
            Duration::try_from_secs_f64(seconds).map_err(|_| ReadinessRefusal::Interval)?;
        Ok(Self {
            attempts,
            interval,
            probe_timeout,
        })
    }
}
pub fn ssh_ports(output: &[u8]) -> Result<Vec<u16>, ReadinessRefusal> {
    let mut ports = Vec::new();
    for line in output.split(|byte| *byte == b'\n') {
        let mut fields = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|part| !part.is_empty());
        if fields.next() != Some(b"port") {
            continue;
        }
        let value = fields.next().unwrap_or_default();
        let port = std::str::from_utf8(value)
            .ok()
            .and_then(|v| canonical_integer(v, 5))
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| ReadinessRefusal::Port(value.to_vec()))?;
        if !ports.contains(&port) {
            ports.push(port);
        }
    }
    if ports.is_empty() {
        Err(ReadinessRefusal::MissingPort)
    } else {
        Ok(ports)
    }
}
pub fn has_host_key(output: &[u8]) -> bool {
    output.split(|byte| *byte == b'\n').any(|line| {
        let mut fields = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|part| !part.is_empty());
        fields.next().is_some_and(|host| !host.starts_with(b"#"))
            && fields.next().is_some()
            && fields.next().is_some()
    })
}
pub fn ssh_verify_deadline(value: &str) -> Duration {
    Duration::from_secs(u64::from(
        canonical_integer(value, 5)
            .filter(|v| *v <= 86400)
            .unwrap_or(120),
    ))
}

fn canonical_integer(value: &str, width: usize) -> Option<u32> {
    if value.is_empty()
        || value.starts_with('0')
        || value.len() > width
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests;
