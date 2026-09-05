//! `[lanes.brew]`: Homebrew formulae, casks and Mac App Store apps, plus the
//! two repairs only that lane is positioned to make.
//!
//! EVERY PATH IS A KEY, and every key ships at its default, because a machine
//! whose Homebrew prefix or state directory differs has nothing else to
//! change. The three under the operator's home CANNOT carry a default here
//! (this module never reads the environment), so an empty one turns its step
//! into a stated skip rather than a guess.

use crate::config::ConfigError;
use crate::config::schema::{admits, non_empty};

#[derive(Debug, Clone, PartialEq)]
pub struct BrewLane {
    pub brew: String,
    pub mas: String,
    pub tailscaled: String,
    pub osquery_converge: String,
    pub mas_manifest: String,
    pub upgrade_record: String,
}

/// The Homebrew commands when no key states them.
pub const DEFAULT_BREW: &str = "/opt/homebrew/bin/brew";
pub const DEFAULT_MAS: &str = "/opt/homebrew/bin/mas";
/// The build `brew upgrade` moves, which is NOT the copy the system daemon
/// runs; the lane's own step is what reconciles the two.
pub const DEFAULT_TAILSCALED: &str = "/opt/homebrew/opt/tailscale/bin/tailscaled";

impl Default for BrewLane {
    fn default() -> Self {
        BrewLane {
            brew: DEFAULT_BREW.to_string(),
            mas: DEFAULT_MAS.to_string(),
            tailscaled: DEFAULT_TAILSCALED.to_string(),
            osquery_converge: String::new(),
            mas_manifest: String::new(),
            upgrade_record: String::new(),
        }
    }
}

pub(super) fn parse_brew_lane(
    table_label: &str,
    table: toml::Table,
) -> Result<BrewLane, ConfigError> {
    let mut lane = BrewLane::default();
    for (name, setting) in table {
        admits(table_label, "lanes.brew", &name)?;
        match name.as_str() {
            "brew" => lane.brew = non_empty(table_label, &name, &setting)?,
            "mas" => lane.mas = non_empty(table_label, &name, &setting)?,
            "tailscaled" => lane.tailscaled = non_empty(table_label, &name, &setting)?,
            "osquery_converge" => lane.osquery_converge = non_empty(table_label, &name, &setting)?,
            "mas_manifest" => lane.mas_manifest = non_empty(table_label, &name, &setting)?,
            "upgrade_record" => lane.upgrade_record = non_empty(table_label, &name, &setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaneKind;
    use crate::config::probes::{kind, parsed};

    #[test]
    fn a_brew_lane_that_states_no_path_runs_at_the_defaults_the_template_ships() {
        let config = parsed("[lanes.brew]\n");
        let Some(LaneKind::Brew(lane)) = kind(&config, "brew") else {
            panic!("expected a brew lane");
        };
        assert_eq!(lane.brew, DEFAULT_BREW);
        assert_eq!(lane.mas, DEFAULT_MAS);
        assert_eq!(lane.tailscaled, DEFAULT_TAILSCALED);
        // The three under the operator's home have no default to guess.
        assert_eq!(lane.osquery_converge, "");
        assert_eq!(lane.mas_manifest, "");
        assert_eq!(lane.upgrade_record, "");
    }

    #[test]
    fn every_brew_path_the_block_states_is_the_one_the_lane_carries() {
        let config = parsed(
            "[lanes.brew]\nbrew = \"/b\"\nmas = \"/m\"\ntailscaled = \"/t\"\n\
             osquery_converge = \"/c\"\nmas_manifest = \"/f\"\nupgrade_record = \"/r\"\n",
        );
        let Some(LaneKind::Brew(lane)) = kind(&config, "brew") else {
            panic!("expected a brew lane");
        };
        assert_eq!(
            (
                lane.brew.as_str(),
                lane.mas.as_str(),
                lane.tailscaled.as_str(),
                lane.osquery_converge.as_str(),
                lane.mas_manifest.as_str(),
                lane.upgrade_record.as_str()
            ),
            ("/b", "/m", "/t", "/c", "/f", "/r")
        );
    }
}
