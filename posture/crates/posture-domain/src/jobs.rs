//! What each of posture's scheduled jobs is, as a launchd unit.
//!
//! THE SCHEDULE IS PART OF THE PRODUCT. Every posture subcommand samples the
//! current state and exits, so something outside the binary has to start it.
//! On a machine whose installer ships plists of its own, that something is the
//! installer; on every other machine it is this module, and without it an
//! installed posture carries every check and no schedule at all.
//!
//! LAUNCHD RATHER THAN A SLEEP LOOP, deliberately. `man 5 launchd.plist`
//! documents `StartCalendarInterval` as starting a missed fire when the
//! machine wakes, which the two daily jobs depend on, and a watchdog in the
//! same process as the monitors it judges cannot report that it died.
//!
//! The rendering here is a total function of a plan. Writing a unit, loading
//! it and reading launchd back is the composition root's business.

use crate::{Agent, AgentLabels};
use std::path::{Path, PathBuf};

/// The log directory of posture's own jobs, under the home directory. Named
/// for the program whose output it holds rather than for the dependency that
/// program reads.
const LOG_DIRECTORY: &str = ".local/log/posture";

/// The result log osqueryd writes and `posture alert` reads, which is also the
/// path a write to triggers the alert job.
const RESULTS_LOG: &str = ".local/log/osquery/osqueryd.results.log";

/// The search path a launchd job inherits, which is otherwise bare enough that
/// a tool posture shells out to is not found at all.
const JOB_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

/// Time of day a daily job fires, in the machine's local time, the way
/// launchd reads a calendar interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyTime {
    pub hour: u8,
    pub minute: u8,
}

impl DailyTime {
    /// `HH:MM`, refusing anything else by name. A time nothing can be inferred
    /// from is a refusal rather than a silent midnight: a digest that fires at
    /// an hour the operator did not choose is indistinguishable from one that
    /// never fires.
    pub fn parse(text: &str) -> Result<Self, String> {
        let refusal = || format!("`{text}` is not a time of day written as HH:MM");
        let (hour, minute) = text.split_once(':').ok_or_else(refusal)?;
        let field = |value: &str, limit: u8| {
            (value.len() == 2 && value.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.parse::<u8>().ok())
                .flatten()
                .filter(|value| *value < limit)
                .ok_or_else(refusal)
        };
        Ok(Self {
            hour: field(hour, 24)?,
            minute: field(minute, 60)?,
        })
    }
}

/// When the two daily jobs fire. Shipped values are a working schedule rather
/// than a placeholder, so an install with no config still reports and digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyTimes {
    pub digest: DailyTime,
    pub heartbeat: DailyTime,
}

impl Default for DailyTimes {
    fn default() -> Self {
        Self {
            digest: DailyTime {
                hour: 18,
                minute: 0,
            },
            heartbeat: DailyTime { hour: 9, minute: 0 },
        }
    }
}

/// What starts a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Every `seconds`, which launchd counts from the load of the job.
    Every(u32),
    /// Once a day at a stated time, started on wake when the machine slept
    /// through it.
    Daily(DailyTime),
}

/// One job, fully decided: every value the unit needs and nothing read from
/// the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPlan {
    pub label: String,
    pub subcommand: &'static str,
    pub program: PathBuf,
    pub trigger: Trigger,
    /// A file whose change also starts the job. `posture alert` reads the
    /// result log, so a write to it is worth a run before the interval.
    pub watch: Option<PathBuf>,
    pub log: PathBuf,
    /// Whether loading the job also runs it once. The interval jobs sample
    /// live state, so the first sample is wanted immediately; a daily job
    /// asked for a time of day, and the file-watched one waits for a write.
    pub run_at_load: bool,
}

impl JobPlan {
    /// The plan of every job posture installs, in `Agent::ALL` order.
    pub fn all(
        labels: &AgentLabels,
        daily: DailyTimes,
        program: &Path,
        home: &Path,
    ) -> Vec<JobPlan> {
        Agent::ALL
            .iter()
            .map(|agent| Self::new(*agent, labels, daily, program, home))
            .collect()
    }

    pub fn new(
        agent: Agent,
        labels: &AgentLabels,
        daily: DailyTimes,
        program: &Path,
        home: &Path,
    ) -> Self {
        let trigger = match agent {
            Agent::Poll | Agent::Funnel => Trigger::Every(60),
            Agent::Alert => Trigger::Every(300),
            Agent::Watchdog => Trigger::Every(900),
            Agent::Digest => Trigger::Daily(daily.digest),
            Agent::Heartbeat => Trigger::Daily(daily.heartbeat),
        };
        Self {
            label: labels.label(agent).to_string(),
            subcommand: agent.key(),
            program: program.to_path_buf(),
            trigger,
            watch: matches!(agent, Agent::Alert).then(|| home.join(RESULTS_LOG)),
            log: home
                .join(LOG_DIRECTORY)
                .join(format!("{}.log", agent.key())),
            run_at_load: matches!(agent, Agent::Poll | Agent::Funnel | Agent::Watchdog),
        }
    }

    /// Where the unit belongs, which launchd derives from the label rather
    /// than from the job.
    pub fn unit_path(&self, home: &Path) -> PathBuf {
        home.join("Library/LaunchAgents")
            .join(format!("{}.plist", self.label))
    }

    /// The unit, as launchd's own XML property list.
    pub fn unit(&self) -> String {
        let mut text = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \
             \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
        );
        let mut key = |name: &str, body: &str| {
            text.push_str(&format!("  <key>{name}</key>\n{body}"));
        };
        key(
            "Label",
            &format!("  <string>{}</string>\n", escape(&self.label)),
        );
        key(
            "ProgramArguments",
            &format!(
                "  <array>\n    <string>{}</string>\n    <string>{}</string>\n  </array>\n",
                escape(&self.program.to_string_lossy()),
                self.subcommand
            ),
        );
        key(
            "EnvironmentVariables",
            &format!("  <dict>\n    <key>PATH</key>\n    <string>{JOB_PATH}</string>\n  </dict>\n"),
        );
        if let Some(watch) = &self.watch {
            key(
                "WatchPaths",
                &format!(
                    "  <array>\n    <string>{}</string>\n  </array>\n",
                    escape(&watch.to_string_lossy())
                ),
            );
        }
        key(
            "RunAtLoad",
            if self.run_at_load {
                "  <true/>\n"
            } else {
                "  <false/>\n"
            },
        );
        match self.trigger {
            Trigger::Every(seconds) => {
                key(
                    "StartInterval",
                    &format!("  <integer>{seconds}</integer>\n"),
                );
            }
            Trigger::Daily(DailyTime { hour, minute }) => key(
                "StartCalendarInterval",
                &format!(
                    "  <dict>\n    <key>Hour</key><integer>{hour}</integer>\n    \
                     <key>Minute</key><integer>{minute}</integer>\n  </dict>\n"
                ),
            ),
        }
        let log = format!(
            "  <string>{}</string>\n",
            escape(&self.log.to_string_lossy())
        );
        key("StandardOutPath", &log);
        key("StandardErrorPath", &log);
        text.push_str("</dict>\n</plist>\n");
        text
    }

    /// How the schedule reads on one line, for the operator rather than for
    /// launchd.
    pub fn schedule(&self) -> String {
        let schedule = match self.trigger {
            Trigger::Every(seconds) => format!("every {seconds}s"),
            Trigger::Daily(DailyTime { hour, minute }) => format!("daily at {hour:02}:{minute:02}"),
        };
        match &self.watch {
            Some(watch) => format!("{schedule}, and on a write to {}", watch.display()),
            None => schedule,
        }
    }
}

/// XML text is the operator's own label and paths, so the five predefined
/// entities are the trust boundary between a config value and a parsable unit.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// How a unit on disk differs from the one posture would write, as the lines
/// each side holds alone.
///
/// LINES RATHER THAN PARSED KEYS. The comparison exists so an operator whose
/// units were written by something else (this repository's own templates, for
/// one) can see what actually differs, and a line of launchd XML says which
/// key and which value in one string. Whitespace and blank lines are ignored,
/// so only content differs; ordering is not, so a reordered unit reads as
/// equal.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UnitDrift {
    /// Lines posture would write that the unit on disk does not hold.
    pub expected: Vec<String>,
    /// Lines the unit on disk holds that posture would not write.
    pub live: Vec<String>,
}

impl UnitDrift {
    pub fn is_empty(&self) -> bool {
        self.expected.is_empty() && self.live.is_empty()
    }
}

/// The two-way difference between what posture would write and what is there.
pub fn unit_drift(expected: &str, live: &str) -> UnitDrift {
    let (expected, live) = (content_lines(expected), content_lines(live));
    UnitDrift {
        expected: difference(&expected, &live),
        live: difference(&live, &expected),
    }
}

/// Every line with content, sorted, so a reordered unit reads as equal.
fn content_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    lines.sort();
    lines
}

/// The lines of `from` that `other` does not hold, counting repeats, so two
/// identical log-path lines stay two.
fn difference(from: &[String], other: &[String]) -> Vec<String> {
    let mut remaining = other.to_vec();
    from.iter()
        .filter(
            |line| match remaining.iter().position(|held| held == *line) {
                Some(index) => {
                    remaining.remove(index);
                    false
                }
                None => true,
            },
        )
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
