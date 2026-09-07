//! Package-owned parser fixtures captured from the pre-extraction template.
//! Historical leaf names remain so the test-successor map stays intact.

use super::*;
use crate::config::probes::{checked_text, parse_config, typed};

const CONFIG: &str = include_str!("fixtures/config.toml");
const COMMAND: &str = include_str!("fixtures/command.toml");

#[test]
fn the_shipped_template_still_parses_and_selects_what_it_selects() {
    let config = parse_config(CONFIG)
        .unwrap_or_else(|error| panic!("the parser fixture must load: {error:?}"));
    assert!(config.records.is_some());
    assert!(config.alerts.is_some());
    assert_eq!(
        typed::<HerdrLane>(CONFIG, "herdr"),
        Some(HerdrLane {
            binary: "/stand-in/.local/bin/herdr".to_string(),
            plugins: vec![Plugin {
                id: "stand-in".to_string(),
                repo: "stand-in".to_string(),
            }],
        })
    );
    // THE LANE THE FILE TURNS ON, not only that the file loads. A block
    // dropped from the template leaves a machine whose global packages
    // quietly stop being upgraded, and a parse that still succeeds is
    // exactly what makes that invisible.
    assert_eq!(
        typed::<NpmLane>(CONFIG, "npm"),
        Some(NpmLane {
            binary: "/stand-in/.local/share/fnm/aliases/default/bin/npm".to_string(),
        })
    );
    // AND THE SAME FOR THE OTHER LANE: a block dropped from the template
    // leaves a machine whose uv tools quietly stop being upgraded, and a
    // parse that still succeeds is exactly what makes that invisible.
    assert_eq!(
        typed::<UvLane>(CONFIG, "uv"),
        Some(UvLane {
            binary: "/opt/homebrew/bin/uv".to_string(),
        })
    );
    // AND THE LANE THAT CARRIES THE REPAIRS. Dropping this block costs
    // more than upgrades: the osquery converge and the upgrade record the
    // file-integrity page correlates against both live inside it, so the
    // machine would run a root daemon on the vendor default config after
    // the next cask upgrade with nothing saying so. EVERY PATH IS
    // ASSERTED, because a key silently missing from the block is how one
    // step turns into a stated skip nobody reads.
    assert_eq!(
        typed::<BrewLane>(CONFIG, "brew"),
        Some(BrewLane {
            brew: DEFAULT_BREW.to_string(),
            mas: DEFAULT_MAS.to_string(),
            tailscaled: DEFAULT_TAILSCALED.to_string(),
            osquery_converge: "/stand-in/.local/libexec/osquery/osquery-converge.sh".to_string(),
            mas_manifest: "/stand-in/.local/state/homebrew/mas.Brewfile".to_string(),
            upgrade_record:
                "/stand-in/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv"
                    .to_string(),
        })
    );
}

#[test]
fn the_templates_command_example_still_loads_once_uncommented() {
    checked_text(COMMAND);
    assert_eq!(
        typed::<CommandLane>(COMMAND, "example"),
        Some(CommandLane {
            run: vec!["/usr/local/bin/my-updater".to_string(), "--yes".to_string()],
        })
    );
}
