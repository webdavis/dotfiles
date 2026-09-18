//! The config file's shape, declared once so serde refuses a malformed value
//! and an unknown mode by name, and names every key it does not read.
//!
//! A KEY THIS BUILD DOES NOT READ IS A WARNING, A BROKEN VALUE IS A REFUSAL.
//! Voiding the whole file over one unread word takes every destination away
//! and leaves a fail-closed default that delivers nothing, so a config written
//! for a newer or older build of this tool keeps working and says what was
//! ignored. A value a known key cannot hold is different: nothing can be
//! inferred about what the operator meant, so it still blocks the file.

use super::{COPY_WINDOW, DEFAULT_ROUTE, DEFAULT_WEBHOOK_BASE, NotifyMode};
use crate::hermes::CriticalCopy;
use crate::wire::Name;
use posture_domain::{Agent, AgentLabels, DailyTime, DailyTimes};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub(super) struct File {
    pub(super) notify: Table,
    /// The launchd label of each job posture installs. Absent, and absent per
    /// key, is the shipped default for that job.
    #[serde(default)]
    pub(super) jobs: Jobs,
    #[serde(flatten)]
    unread: Unread,
}

/// ONE KEY PER JOB, each naming posture's own subcommand, so a label states
/// what the job DOES rather than what it reads. Whoever installs posture
/// chooses the labels; a name compiled into this tool would only ever match the
/// machine it was compiled for.
#[derive(Default, Deserialize)]
pub(super) struct Jobs {
    watchdog: Option<String>,
    alert: Option<String>,
    poll: Option<String>,
    funnel: Option<String>,
    digest: Option<String>,
    heartbeat: Option<String>,
    /// When the two daily jobs fire. Absent is the shipped schedule.
    #[serde(default)]
    daily: Daily,
    #[serde(flatten)]
    unread: Unread,
}

/// THE TIME OF DAY OF EACH DAILY JOB, stated as `HH:MM` in the machine's own
/// local time. A job that fires once a day has to be told when, and the
/// installer of a product cannot read the data files of whoever packaged it.
#[derive(Default, Deserialize)]
pub(super) struct Daily {
    digest: Option<String>,
    heartbeat: Option<String>,
    #[serde(flatten)]
    unread: Unread,
}

impl Jobs {
    /// The defaults with every stated label written over them.
    pub(super) fn into_labels(self) -> AgentLabels {
        let mut labels = AgentLabels::default();
        let stated = [
            (Agent::Watchdog, self.watchdog),
            (Agent::Alert, self.alert),
            (Agent::Poll, self.poll),
            (Agent::Funnel, self.funnel),
            (Agent::Digest, self.digest),
            (Agent::Heartbeat, self.heartbeat),
        ];
        for (agent, label) in stated {
            if let Some(label) = label.filter(|label| !label.is_empty()) {
                labels.set(agent, label);
            }
        }
        labels
    }

    /// The daily times, with an unparsable one left at its default and named
    /// as a warning. A time the operator wrote wrong is worth saying out loud;
    /// voiding the file over it would take every label with it.
    pub(super) fn times(&self) -> (DailyTimes, Vec<String>) {
        let mut times = DailyTimes::default();
        let mut warnings = Vec::new();
        let mut read = |key: &str, stated: &Option<String>, field: &mut DailyTime| {
            let Some(text) = stated.as_ref().filter(|text| !text.is_empty()) else {
                return;
            };
            match DailyTime::parse(text) {
                Ok(time) => *field = time,
                Err(refusal) => warnings.push(format!(
                    "`jobs.daily.{key}` is ignored and the shipped time is used: {refusal}"
                )),
            }
        };
        read("digest", &self.daily.digest, &mut times.digest);
        read("heartbeat", &self.daily.heartbeat, &mut times.heartbeat);
        (times, warnings)
    }
}

#[derive(Deserialize)]
pub(super) struct Table {
    mode: Mode,
    command: Option<Command>,
    hermes: Option<Hermes>,
    /// The route every page with no tier of its own takes, whichever mode
    /// carries it. `Name` refuses a control character and bounds the length,
    /// so what is left is used as a path segment exactly as written, the same
    /// trust boundary as `hermes.url`.
    #[serde(default)]
    route: Option<Name>,
    #[serde(flatten)]
    unread: Unread,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Hermes,
    Command,
    Off,
}

#[derive(Deserialize)]
struct Command {
    path: PathBuf,
    #[serde(default)]
    arguments: Vec<String>,
    #[serde(flatten)]
    unread: Unread,
}

#[derive(Deserialize)]
struct Hermes {
    #[serde(default = "local_gateway")]
    url: String,
    #[serde(default)]
    keys: BTreeMap<String, String>,
    /// A ROUTE NAME, AND NOTHING ABOUT WHAT READS IT. A page whose tier is
    /// critical is copied here verbatim once its own post came back
    /// delivered; absent is one post. `Name` only bounds length and refuses
    /// control characters, so it is used as a path segment exactly as
    /// written; the value comes from the operator's own config, the same
    /// trust boundary as the `url` key beside it.
    #[serde(default)]
    critical_copy_route: Option<Name>,
    #[serde(flatten)]
    unread: Unread,
}

fn local_gateway() -> String {
    DEFAULT_WEBHOOK_BASE.to_string()
}

/// Whatever a table held that this build has no field for, kept by name so it
/// can be reported instead of discarded. Collected by serde itself rather than
/// checked against a hand-written list of known keys, which would be a second
/// copy of every field name above.
type Unread = BTreeMap<String, toml::Value>;

impl File {
    /// One line per key nothing in this build reads, named with its table so
    /// the operator can find it in the file.
    pub(super) fn unread_keys(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut named = |prefix: &str, unread: &Unread| {
            lines.extend(unread.keys().map(|key| {
                format!("`{prefix}{key}` is not a key this build reads, so it is ignored")
            }));
        };
        named("", &self.unread);
        named("notify.", &self.notify.unread);
        if let Some(command) = &self.notify.command {
            named("notify.command.", &command.unread);
        }
        if let Some(hermes) = &self.notify.hermes {
            named("notify.hermes.", &hermes.unread);
        }
        named("jobs.", &self.jobs.unread);
        named("jobs.daily.", &self.jobs.daily.unread);
        lines
    }
}

impl Table {
    /// The stated untiered route, or the shipped default.
    pub(super) fn route(&self) -> String {
        self.route
            .as_ref()
            .map(|route| route.as_str().to_string())
            .unwrap_or_else(|| DEFAULT_ROUTE.to_string())
    }

    /// The declared mode, or the refusal for a mode whose own table is missing
    /// what that mode cannot run without.
    pub(super) fn into_mode(self, home: &Path) -> Result<NotifyMode, String> {
        match self.mode {
            // A COMMAND MODE WITH NO COMMAND IS REFUSED, not defaulted to some
            // engine's name. Which program serves the contract is the
            // operator's choice, and guessing one would page nowhere while
            // looking configured.
            Mode::Command => {
                let command = self.command.ok_or_else(|| {
                    "`notify.mode` is \"command\" but no `[notify.command]` table states its \
                     `path`"
                        .to_string()
                })?;
                Ok(NotifyMode::Command {
                    path: command.path,
                    arguments: command.arguments,
                })
            }
            Mode::Hermes => {
                let hermes = self.hermes.unwrap_or(Hermes {
                    url: local_gateway(),
                    keys: BTreeMap::new(),
                    critical_copy_route: None,
                    unread: Unread::new(),
                });
                Ok(NotifyMode::Hermes {
                    base_url: hermes.url,
                    keys: hermes.keys,
                    critical_copy: hermes.critical_copy_route.map(|route| CriticalCopy {
                        route,
                        window: home.join(COPY_WINDOW),
                    }),
                })
            }
            // `off` needs no table of its own: there is nothing to configure
            // about raising the local banner and posting nowhere.
            Mode::Off => Ok(NotifyMode::Off),
        }
    }
}
