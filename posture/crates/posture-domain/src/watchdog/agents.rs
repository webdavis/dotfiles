//! posture's own scheduled jobs, and where their launchd labels come from.
//!
//! A LABEL IS CONFIGURATION, NEVER SOURCE. The labels below name whoever
//! installed posture, not this repository's operator, so the shipped defaults
//! are reverse-DNS names of posture's own jobs and `[jobs]` in
//! `~/.config/posture/config.toml` overrides any of them. A hardcoded label
//! would make the watchdog search launchd for jobs a stranger never installed
//! and then either page every tick or report all clear falsely.

/// One scheduled job, named for what it RUNS rather than what it reads.
///
/// The variants are posture's own subcommands, which is the one axis that
/// cannot drift from the job it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Watchdog,
    Alert,
    Poll,
    Funnel,
    Digest,
    Heartbeat,
}
impl Agent {
    /// Every job posture installs, which is the set whose plists are its own.
    pub const ALL: [Self; 6] = [
        Self::Watchdog,
        Self::Alert,
        Self::Poll,
        Self::Funnel,
        Self::Digest,
        Self::Heartbeat,
    ];
    /// The jobs the watchdog judges. It cannot judge itself: a watchdog that
    /// did not run reports nothing at all, which is what the heartbeat is for.
    pub const MONITORED: [Self; 5] = [
        Self::Alert,
        Self::Poll,
        Self::Funnel,
        Self::Digest,
        Self::Heartbeat,
    ];
    /// The job's config key, which is also its state-file key and the last
    /// segment of its default label. STABLE ACROSS A RENAME: keying remembered
    /// state by the job rather than by its label means an operator who renames
    /// a job keeps that job's history.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Watchdog => "watchdog",
            Self::Alert => "alert",
            Self::Poll => "poll",
            Self::Funnel => "funnel",
            Self::Digest => "digest",
            Self::Heartbeat => "heartbeat",
        }
    }
}
/// `AgentLabels` indexes by discriminant (`agent as usize`), so a reordering
/// of `Agent::ALL` that drifted from declaration order would silently
/// mislabel every job. This fails the build instead of waiting on a test.
const _: () = {
    let mut i = 0;
    while i < Agent::ALL.len() {
        assert!(
            Agent::ALL[i] as usize == i,
            "Agent::ALL must stay in discriminant order"
        );
        i += 1;
    }
};

/// The launchd label of each job, defaulted and overridable per job.
///
/// Reverse-DNS is the macOS convention for a launchd label, and the domain
/// names the tool so that a label says which program owns the job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLabels([String; Agent::ALL.len()]);

impl Default for AgentLabels {
    fn default() -> Self {
        Self(Agent::ALL.map(|agent| format!("dev.posture.{}", agent.key())))
    }
}

impl AgentLabels {
    /// The label configured for one job. Indexed by the variant's own
    /// discriminant, which `Agent::ALL` is declared in the order of; the
    /// agreement is pinned by a test.
    pub fn label(&self, agent: Agent) -> &str {
        &self.0[agent as usize]
    }
    /// Replace one job's label, as a config key does.
    pub fn set(&mut self, agent: Agent, label: String) {
        self.0[agent as usize] = label;
    }
    /// Every configured label, which is the set that says whether a plist on
    /// disk belongs to posture.
    pub fn all(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
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
pub fn judge_agent(label: &str, reading: AgentReading<'_>, prior: AgentState) -> AgentJudgment {
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
