//! The config file's shape, declared once so serde refuses an unknown table,
//! an unknown key and an unknown mode by name. A typo therefore blocks the
//! whole file rather than quietly switching delivery off, which is the trade:
//! the loud half is a named refusal, and the quiet half it replaces is a
//! security pipeline that silently stops paging.

use super::{DEFAULT_WEBHOOK_BASE, DeliveryPath};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct File {
    pub(super) delivery: Table,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Table {
    mode: Mode,
    producer: Option<Producer>,
    hermes: Option<Hermes>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Producer,
    Hermes,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    command: PathBuf,
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
}

fn local_gateway() -> String {
    DEFAULT_WEBHOOK_BASE.to_string()
}

impl Table {
    /// The declared path, or the refusal for a mode whose own table is missing
    /// what that mode cannot run without.
    pub(super) fn into_path(self) -> Result<DeliveryPath, String> {
        match self.mode {
            // A PRODUCER WITH NO COMMAND IS REFUSED, not defaulted to some
            // engine's name. Which program serves the contract is the
            // operator's choice, and guessing one would page nowhere while
            // looking configured.
            Mode::Producer => {
                let producer = self.producer.ok_or_else(|| {
                    "`delivery.mode` is \"producer\" but no `[delivery.producer]` table states \
                     its `command`"
                        .to_string()
                })?;
                Ok(DeliveryPath::Producer {
                    command: producer.command,
                    arguments: producer.arguments,
                })
            }
            Mode::Hermes => {
                let hermes = self.hermes.unwrap_or(Hermes {
                    url: local_gateway(),
                    keys: BTreeMap::new(),
                });
                Ok(DeliveryPath::Hermes {
                    base_url: hermes.url,
                    keys: hermes.keys,
                })
            }
        }
    }
}
