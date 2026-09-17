//! The config file's shape, declared once so serde refuses an unknown table,
//! an unknown key and an unknown mode by name. A typo therefore blocks the
//! whole file rather than quietly switching notification off, which is the
//! trade: the loud half is a named refusal, and the quiet half it replaces is
//! a security pipeline that silently stops paging.

use super::{COPY_WINDOW, DEFAULT_WEBHOOK_BASE, NotifyMode};
use crate::hermes::CriticalCopy;
use crate::wire::Name;
use posture_domain::{Agent, AgentLabels};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct File {
    pub(super) notify: Table,
    /// The launchd label of each job posture installs. Absent, and absent per
    /// key, is the shipped default for that job.
    #[serde(default)]
    pub(super) jobs: Jobs,
}

/// ONE KEY PER JOB, each naming posture's own subcommand, so a label states
/// what the job DOES rather than what it reads. Whoever installs posture
/// chooses the labels; a name compiled into this tool would only ever match the
/// machine it was compiled for.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Jobs {
    watchdog: Option<String>,
    alert: Option<String>,
    poll: Option<String>,
    funnel: Option<String>,
    digest: Option<String>,
    heartbeat: Option<String>,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Table {
    mode: Mode,
    command: Option<Command>,
    hermes: Option<Hermes>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Hermes,
    Command,
    Off,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    path: PathBuf,
    #[serde(default)]
    arguments: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
}

fn local_gateway() -> String {
    DEFAULT_WEBHOOK_BASE.to_string()
}

impl Table {
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
