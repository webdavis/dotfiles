//! Does the config this repo ships still load, and still select what it says
//! it selects?
//!
//! `#[cfg(test)]` ONLY, so the binary the apply builds out of the deployed
//! crate never asks for a file that is not there (the same arrangement pns
//! uses for its template): this reaches five levels out of the crate into the
//! repo checkout around it, which only works from inside this repo, and stops
//! compiling the day uu moves to its own repo (see pns's config.rs for the
//! full reasoning, not duplicated here).
//!
//! The two helpers below share the one file: one stands in for the template's
//! chezmoi actions, the other uncomments its command example; neither reaches
//! into the other's part of the file.

use super::*;
use crate::config::probes::{kind, parsed};

const SHIPPED_TEMPLATE: &str =
    include_str!("../../../../../dot_config/uu/private_config.toml.tmpl");

/// The template with its chezmoi actions stood in for: a `{{-` directive
/// standing on its own line goes with the line (the plugin `range` and its
/// `end`), a `| quote` action becomes a quoted stand-in, and any other
/// action becomes a bare one, which is what the two inside a `"..."` need.
fn rendered_template() -> String {
    SHIPPED_TEMPLATE
        .lines()
        .filter(|line| !line.trim_start().starts_with("{{-"))
        .map(|line| {
            let mut rendered = line.to_string();
            while let Some(start) = rendered.find("{{") {
                let end = start + rendered[start..].find("}}").expect("a closed action") + 2;
                // ABSOLUTE, because every bare action in the template is
                // `.chezmoi.homeDir` and a home directory always is; a
                // relative stand-in would fail a key that requires a full
                // path for a reason the real render never has.
                let stand_in = if rendered[start..end].contains("| quote") {
                    "\"stand-in\""
                } else {
                    "/stand-in"
                };
                rendered.replace_range(start..end, stand_in);
            }
            rendered
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_shipped_template_still_parses_and_selects_what_it_selects() {
    // The only uu config anyone has is this file. A key the template
    // writes that the parser refuses blocks the whole weekly job at load,
    // and nothing but this test says so before an apply.
    let config = parse_config(&rendered_template())
        .unwrap_or_else(|error| panic!("the shipped template must load: {error:?}"));
    assert!(config.records.is_some());
    assert!(config.alerts.is_some());
    assert_eq!(
        kind(&config, "herdr"),
        Some(&LaneKind::Herdr(HerdrLane {
            binary: "/stand-in/.local/bin/herdr".to_string(),
            plugins: vec![Plugin {
                id: "stand-in".to_string(),
                repo: "stand-in".to_string(),
            }],
        }))
    );
    // THE LANE THE FILE TURNS ON, not only that the file loads. A block
    // dropped from the template leaves a machine whose global packages
    // quietly stop being upgraded, and a parse that still succeeds is
    // exactly what makes that invisible.
    assert_eq!(
        kind(&config, "npm"),
        Some(&LaneKind::Npm(NpmLane {
            binary: "/stand-in/.local/share/fnm/aliases/default/bin/npm".to_string(),
        }))
    );
    // AND THE SAME FOR THE OTHER LANE: a block dropped from the template
    // leaves a machine whose uv tools quietly stop being upgraded, and a
    // parse that still succeeds is exactly what makes that invisible.
    assert_eq!(
        kind(&config, "uv"),
        Some(&LaneKind::Uv(UvLane {
            binary: "/opt/homebrew/bin/uv".to_string(),
        }))
    );
    // AND THE LANE THAT CARRIES THE REPAIRS. Dropping this block costs
    // more than upgrades: the osquery converge and the upgrade record the
    // file-integrity page correlates against both live inside it, so the
    // machine would run a root daemon on the vendor default config after
    // the next cask upgrade with nothing saying so. EVERY PATH IS
    // ASSERTED, because a key silently missing from the block is how one
    // step turns into a stated skip nobody reads.
    assert_eq!(
        kind(&config, "brew"),
        Some(&LaneKind::Brew(BrewLane {
            brew: DEFAULT_BREW.to_string(),
            mas: DEFAULT_MAS.to_string(),
            tailscaled: DEFAULT_TAILSCALED.to_string(),
            osquery_converge: "/stand-in/.local/libexec/osquery/osquery-converge.sh".to_string(),
            mas_manifest: "/stand-in/.local/state/homebrew/mas.Brewfile".to_string(),
            upgrade_record:
                "/stand-in/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv"
                    .to_string(),
        }))
    );
}

/// The template's commented `[lanes.example]` block, uncommented by
/// stripping each line's leading `#`: what an operator gets after
/// following the template's own instruction to "uncomment and rename the
/// block." The block holds no chezmoi action, so unlike `rendered_template`
/// above, nothing needs a stand-in before parsing.
fn shipped_command_example_uncommented() -> String {
    SHIPPED_TEMPLATE
        .lines()
        .skip_while(|line| line.trim() != "# [lanes.example]")
        .take_while(|line| line.trim_start().starts_with('#'))
        .map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix("# ")
                .or_else(|| trimmed.strip_prefix('#'))
                .unwrap_or(trimmed)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_templates_command_example_still_loads_once_uncommented() {
    // Read from the shipped file itself, not transcribed: an edit to the
    // template that breaks the example (like dropping `type =
    // "command"`) fails this test without anyone keeping a copy here in
    // sync.
    let config = parsed(&shipped_command_example_uncommented());
    assert_eq!(
        kind(&config, "example"),
        Some(&LaneKind::Command(CommandLane {
            run: vec!["/usr/local/bin/my-updater".to_string(), "--yes".to_string()],
        }))
    );
}
