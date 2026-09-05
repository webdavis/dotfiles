//! `[lanes]`: the REGISTRY, whose blocks are keyed by an operator-chosen NAME
//! and dispatch on the TYPE each one states.
//!
//! A TYPE IS REQUIRED, EXPLICIT OR IMPLIED, and an unrecognized one is
//! refused rather than ignored, which is the one place this file departs
//! from pns's free-form plugin settings. A lane declared but never dispatched
//! is a subject that silently never updates, and nothing else on the machine
//! would ever say so.
//!
//! ONE MODULE PER KIND, mirroring `crate::lanes`: what a `[lanes.npm]` block
//! parses into lives in `config::lanes::npm`, and what running that lane does
//! lives in `lanes::npm`.

mod brew;
mod command;
mod herdr;
mod npm;
mod uv;

use std::collections::BTreeMap;
use std::time::Duration;

use super::ConfigError;
use super::schema::{non_empty, table_of};
use crate::deadline::{DEFAULT_LANE_DEADLINE, parse_deadline};

pub use brew::{BrewLane, DEFAULT_BREW, DEFAULT_MAS, DEFAULT_TAILSCALED};
pub use command::CommandLane;
pub use herdr::{DEFAULT_HERDR_BINARY, HerdrLane, Plugin};
pub use npm::NpmLane;
pub use uv::{DEFAULT_UV_BINARY, UvLane};

/// The lane TYPES this build knows how to run: the roster of BUILT-IN
/// adapters, never the roster of names an operator may declare. A lane's NAME
/// is the operator's own choice (a producer-API lane names itself whatever it
/// likes); this is the roster its `type` is judged against, so the refusal
/// that names an unknown type and the listing of what this build serves both
/// read the one table.
pub const LANE_TYPES: &[&str] = &["brew", "command", "herdr", "npm", "uv"];

/// The lane REGISTRY: every declared `[lanes.<name>]` block, keyed by the name
/// the operator chose. A `BTreeMap` orders lanes by NAME regardless of the
/// file's own order (`toml::Table` is itself a `BTreeMap`, and neither this
/// crate nor pns enables the `toml`/`indexmap` `preserve_order` feature, so a
/// run's sequence never depends on where a block happens to sit in the file).
pub type Lanes = BTreeMap<String, Lane>;

/// One declared `[lanes.<name>]` block: the adapter that runs it, and the
/// deadline that bounds it.
///
/// THE DEADLINE SITS BESIDE THE KIND RATHER THAN INSIDE IT, so a lane type
/// added later is bounded by construction instead of by its author
/// remembering to carry the field. The run lock is held across lane
/// execution, so a lane with no bound is a lock nothing ever gets back.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub kind: LaneKind,
    pub deadline: Duration,
}

/// One lane's ADAPTER, selected by its `type` key (the house rule: `type`
/// selects backends everywhere). The kind carries the lane's own parsed
/// settings; behavior lives in each kind's `LaneAdapter` impl, never here.
#[derive(Debug, Clone, PartialEq)]
pub enum LaneKind {
    Brew(BrewLane),
    Command(CommandLane),
    Herdr(HerdrLane),
    Npm(NpmLane),
    Uv(UvLane),
}

impl LaneKind {
    /// The `type` value this variant was parsed from.
    pub fn type_name(&self) -> &'static str {
        match self {
            LaneKind::Brew(_) => "brew",
            LaneKind::Command(_) => "command",
            LaneKind::Herdr(_) => "herdr",
            LaneKind::Npm(_) => "npm",
            LaneKind::Uv(_) => "uv",
        }
    }
}

/// Whether a lane's own NAME is safe to use as a directory component. A
/// lane's name flows straight into a path (the streak file the binary keeps)
/// with no other guard between the config and the filesystem, so a name that
/// is not a single plain component could escape the state directory (`..`) or
/// land somewhere unsurprising (an absolute name, a leading `/`). A config
/// already names arbitrary programs to run, so this grants nothing a hostile
/// config does not already have; the point is that an honest TYPO must not
/// be able to truncate an unrelated file.
fn is_plain_path_segment(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && name != "." && name != ".."
}

pub(super) fn parse_lanes(value: toml::Value) -> Result<Lanes, ConfigError> {
    let table = table_of("lanes", value)?;
    let mut lanes = Lanes::new();
    for (name, block) in table {
        if !is_plain_path_segment(&name) {
            return Err(ConfigError::Invalid(format!(
                "lane name `{name}` is not a plain path segment; a lane's own name becomes a \
                 directory under its state path, so it must not contain `/` and must not be `.` \
                 or `..`"
            )));
        }
        let table_label = format!("lanes.{name}");
        let mut fields = table_of(&table_label, block)?;
        // TAKEN BEFORE THE DISPATCH, because every lane type carries it and
        // none of them has an arm to read it: left in the table it would meet
        // each kind parser's do-nothing fallthrough and be silently ignored.
        let deadline = match fields.remove("deadline_secs") {
            Some(stated) => parse_deadline(&table_label, &stated)?,
            None => DEFAULT_LANE_DEADLINE,
        };
        let kind = match lane_type(&name, &table_label, &fields)?.as_str() {
            "brew" => LaneKind::Brew(brew::parse_brew_lane(&table_label, fields)?),
            "command" => LaneKind::Command(command::parse_command_lane(&table_label, fields)?),
            "herdr" => LaneKind::Herdr(herdr::parse_herdr_lane(&table_label, fields)?),
            "npm" => LaneKind::Npm(npm::parse_npm_lane(&table_label, fields)?),
            "uv" => LaneKind::Uv(uv::parse_uv_lane(&table_label, fields)?),
            // `lane_type` never returns anything outside `LANE_TYPES`.
            _ => unreachable!("lane_type only answers a member of LANE_TYPES"),
        };
        lanes.insert(name, Lane { kind, deadline });
    }
    Ok(lanes)
}

/// The lane's TYPE: read from its own `type` key, or IMPLIED when the block
/// says nothing and the NAME itself is a built-in type.
///
/// ONLY A BUILT-IN NAME GETS THIS DEFAULT, narrower on purpose than pns's
/// "NOTHING GUESSES A BACKEND": it is what keeps a config written before the
/// producer API (a bare `[lanes.herdr]`, no `type`) working unchanged. A name
/// that is not a built-in type and states no `type` names nothing to dispatch
/// on, so it is refused rather than guessed.
fn lane_type(name: &str, table_label: &str, table: &toml::Table) -> Result<String, ConfigError> {
    match table.get("type") {
        Some(value) => {
            let stated = non_empty(table_label, "type", value)?;
            if LANE_TYPES.contains(&stated.as_str()) {
                Ok(stated)
            } else {
                Err(ConfigError::Invalid(format!(
                    "lane `{name}` has type `{stated}`, which is no lane type; this build serves \
                     {}",
                    LANE_TYPES.join(", ")
                )))
            }
        }
        None if LANE_TYPES.contains(&name) => Ok(name.to_string()),
        None => Err(ConfigError::Invalid(format!(
            "lane `{name}` names no `type`; this build serves {}",
            LANE_TYPES.join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;
    use crate::config::probes::{kind, parsed, refusal};

    #[test]
    fn a_lane_named_after_no_built_in_type_and_naming_no_type_is_refused_by_name() {
        let detail = refusal("[lanes.hedr]\n");
        assert!(detail.contains("lane `hedr` names no `type`"), "{detail}");
        assert!(detail.contains("herdr"), "{detail}");
    }

    #[test]
    fn a_lane_name_that_is_not_a_plain_path_segment_is_refused_at_load() {
        // Confirmed live (4b's review): `[lanes."../../../../pwned"]` created
        // `$HOME/pwned/streak`, escaping the state directory entirely. A lane
        // name flows straight into a path with no other guard, so a typo
        // must not be able to truncate an unrelated file.
        for name in ["..", ".", "../escaped", "a/b", "/absolute"] {
            let detail = refusal(&format!("[lanes.{name:?}]\ntype = \"herdr\"\n"));
            assert!(
                detail.contains("is not a plain path segment"),
                "case {name:?}: {detail}"
            );
        }
    }

    #[test]
    fn an_ordinary_lane_name_is_unaffected_by_the_path_segment_check() {
        assert!(parse_config("[lanes.mine]\ntype = \"herdr\"\n").is_ok());
        assert!(parse_config("[lanes.my-lane_2]\ntype = \"herdr\"\n").is_ok());
    }

    #[test]
    fn an_unknown_lane_type_is_refused_naming_it_and_the_known_types() {
        let detail = refusal("[lanes.mine]\ntype = \"hedr\"\n");
        assert!(detail.contains("lane `mine` has type `hedr`"), "{detail}");
        assert!(detail.contains("herdr"), "{detail}");
    }

    #[test]
    fn a_block_named_for_one_type_that_states_another_is_the_stated_type() {
        // The stated `type` wins over a name that happens to be a type of its
        // own: the name is the operator's label, the type is the contract.
        assert_eq!(
            kind(
                &parsed("[lanes.herdr]\ntype = \"command\"\nrun = [\"x\"]\n"),
                "herdr"
            ),
            Some(&LaneKind::Command(CommandLane {
                run: vec!["x".to_string()],
            }))
        );
    }

    #[test]
    fn every_built_in_lane_type_has_a_minimal_block_the_parser_accepts() {
        // One minimal, valid block per BUILT-IN TYPE (the WEEKDAY_NAMES
        // pattern): a type in the roster that the parser refuses would
        // advertise a lane nobody can turn on.
        let fixtures: &[(&str, &str)] = &[
            ("brew", "[lanes.brew]\n"),
            ("command", "[lanes.command]\nrun = [\"x\"]\n"),
            ("herdr", "[lanes.herdr]\n"),
            ("npm", "[lanes.npm]\nbinary = \"/n/npm\"\n"),
            ("uv", "[lanes.uv]\n"),
        ];
        assert_eq!(LANE_TYPES.len(), fixtures.len());
        for (lane_type, text) in fixtures {
            let config = parse_config(text).unwrap_or_else(|error| {
                panic!("the roster names `{lane_type}` but the parser refuses its block: {error:?}")
            });
            // AND CALLS ITSELF WHAT THE ROSTER CALLS IT. Each fixture block
            // names its lane after its own type, and `type_name` is the word
            // doctor prints beside the lane, so a kind wired to the wrong
            // literal there tells the operator a lane is something it is not.
            let lane = config
                .lanes
                .get(*lane_type)
                .expect("each fixture names its lane after its type");
            assert_eq!(lane.kind.type_name(), *lane_type);
        }
    }
}
