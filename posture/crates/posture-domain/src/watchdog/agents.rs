#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    ResultsAlerter,
    FirewallGatekeeperMonitor,
    AlertDrainer,
    Digest,
    Heartbeat,
    TailscaleMonitor,
}
impl Agent {
    pub const ALL: [Self; 6] = [
        Self::ResultsAlerter,
        Self::FirewallGatekeeperMonitor,
        Self::AlertDrainer,
        Self::Digest,
        Self::Heartbeat,
        Self::TailscaleMonitor,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::ResultsAlerter => "com.webdavis.osquery-results-alerter",
            Self::FirewallGatekeeperMonitor => "com.webdavis.osquery-firewall-gatekeeper-monitor",
            Self::AlertDrainer => "com.webdavis.osquery-alert-drainer",
            Self::Digest => "com.webdavis.osquery-digest",
            Self::Heartbeat => "com.webdavis.osquery-heartbeat",
            Self::TailscaleMonitor => "com.webdavis.osquery-tailscale-monitor",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode<'a>(&'a str);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentExit<'a> {
    Code(ExitCode<'a>),
    NeverExited,
    Missing,
    Unknown,
}
impl<'a> AgentExit<'a> {
    pub fn from_field(field: Option<&'a str>) -> Self {
        let Some(field) = field else {
            return Self::Missing;
        };
        let field = field.trim_start_matches(|c: char| c.is_ascii_whitespace());
        let sign = usize::from(field.starts_with('-'));
        let digits = field[sign..].bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            return Self::Code(ExitCode(&field[..sign + digits]));
        }
        if field.trim_end_matches(|c: char| c.is_ascii_whitespace()) == "(never exited)" {
            Self::NeverExited
        } else {
            Self::Unknown
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentReading<'a> {
    Unloaded,
    Loaded {
        runs: Option<u64>,
        exit: AgentExit<'a>,
    },
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentState {
    pub runs: Option<u64>,
    pub streak: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct AgentJudgment {
    pub state: Option<AgentState>,
    pub problem: Option<String>,
}
pub fn judge_agent(agent: Agent, reading: AgentReading<'_>, prior: AgentState) -> AgentJudgment {
    let label = agent.label();
    let AgentReading::Loaded { runs, exit } = reading else {
        return AgentJudgment {
            state: None,
            problem: Some(format!("LaunchAgent not loaded: {label}")),
        };
    };
    let mut streak = prior.streak;
    let problem = match exit {
        AgentExit::NeverExited => {
            streak = 0;
            None
        }
        AgentExit::Code(ExitCode(code)) => {
            if code.trim_start_matches('-').bytes().all(|b| b == b'0') {
                streak = 0;
            }
            // A changed counter, including a reset, is a new run in the Bash policy.
            // Unknown runs also accumulate, while a frozen nonzero exit does not.
            else if runs.is_none() || runs != prior.runs {
                streak = streak.saturating_add(1);
            }
            (streak >= 2).then(|| format!("LaunchAgent crash-looping (last exit {code}, {streak} failing re-runs): {label}"))
        }
        AgentExit::Missing => Some(format!(
            "LaunchAgent state is unreadable (launchctl print has no last-exit-code field): {label}"
        )),
        AgentExit::Unknown => Some(format!(
            "LaunchAgent exit state is unreadable (unexpected last-exit-code value): {label}"
        )),
    };
    AgentJudgment {
        state: Some(AgentState { runs, streak }),
        problem,
    }
}
#[cfg(test)]
mod tests;
