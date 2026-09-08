use super::*;

/// The shipped config, which is the config OF RECORD: this repo's template
/// is the only pns config anyone has.
///
/// INCLUDED AT COMPILE TIME AND ONLY UNDER `cfg(test)`, so the binary the
/// apply builds out of the deployed crate (which is the crate alone, no
/// repo around it) never asks for a file that is not there. Measured both
/// ways with the path pointed at a file that does not exist: `cargo build
/// --bin pns` exits 0 because `cfg(test)` is stripped before the macro
/// expands, and `cargo test --no-run` fails with "couldn't read".
///
/// THE COST IS THAT THE TEST BUILD REACHES FOUR LEVELS OUT OF THE CRATE,
/// into the repo checkout around it. `cargo test` and `cargo clippy
/// --all-targets` therefore only work from inside this repo: run either in
/// the deployed `~/.local/share/pns` and the error is a "couldn't read"
/// naming a path, which says nothing about why. THE DAY pns MOVES TO ITS
/// OWN REPO, as it is planned to, this test stops compiling and the
/// template it reads has to arrive by another road (a copy vendored into
/// the crate, or a path handed in by the build). No mechanism is built for
/// that day here; it is written down so it is found by reading rather than
/// by a build breaking.
const SHIPPED_TEMPLATE: &str =
    include_str!("../../../../../dot_config/pns/private_config.toml.tmpl");

/// The one committed input `pns-config-render` walks to produce
/// `SHIPPED_TEMPLATE`. Same four-levels-out caveat as `SHIPPED_TEMPLATE`
/// itself.
const CONFIG_VALUES: &str = include_str!("../../../../../dot_config/pns/config-values.toml");

/// The tables the shipped template leaves LIVE, enumerated rather than
/// counted and kept by hand, because a count cannot say WHICH table went
/// (recurring bug class 10, completeness over counts).
///
/// THE MUTANT THIS PINS is the one the byte-equality test above cannot
/// see. A table ABSENT from the committed values file renders COMMENTED
/// OUT rather than refused, so dropping `[nag]` from that file and
/// running `just pns-config-render` writes a template with the nag the
/// operator runs switched OFF, and the byte-equality test stays green
/// because both sides moved together. Measured: with `[nag]` dropped the
/// whole Rust suite passes. An emptied values file renders exit 0 with
/// four live tables instead of twenty-two. Only a list held OUTSIDE the
/// values file tells a deliberate retirement from an accidental deletion,
/// and this is that list: a table retired on purpose is retired here too,
/// in the same commit, where a reviewer sees it.
///
/// THE CEILING: this pins WHICH tables are live, not what every live key
/// holds. A key dropped from the values file renders at its schema
/// DEFAULT rather than commented, which this does not catch. That case
/// still reads as a changed value in the template's own diff, where a
/// whole table going commented reads as a comment reflow.
const LIVE_TABLES: [&str; 24] = [
    "plugins.mobile",
    "plugins.hermes",
    "plugins.macos-banner",
    "plugins.hue",
    "plugins.presence",
    "plugins.router",
    "daemon",
    "delivery",
    "recap",
    "nag",
    "lights",
    "lights.done",
    "lights.failed",
    "lights.blocked",
    "lights.unread",
    "lights.loop",
    "lights.dim",
    r#"lights.lamp."1F - Front door - HCL1""#,
    r#"lights.lamp."2F - Kitchen - HCD6""#,
    r#"lights.lamp."3F - MBedroom - HCL3""#,
    r#"lights.lamp."3F - Studio - HCL3""#,
    r#"lights.room."2F - Kitchen""#,
    r#"lights.room."3F - MBedroom""#,
    r#"lights.room."3F - Studio""#,
];

/// The last-known-good PARSED configuration, `render(&CONFIG_VALUES)` run
/// through the real `parse_config`, `{:#?}` printed. Committed on purpose
/// SEPARATELY from `config-values.toml` and `SHIPPED_TEMPLATE`, because a
/// snapshot regenerated the same way those two are would move in lockstep
/// with every values-file edit and never disagree with anything.
const RESOLVED_CONFIG_SNAPSHOT: &str =
    include_str!("../../tests/fixtures/resolved-config.snapshot");

/// Absolute so the failure message below can hand back a `cp` command
/// that runs from anywhere, the way `cargo test` output itself does.
const RESOLVED_CONFIG_SNAPSHOT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/resolved-config.snapshot"
);

/// The template with its chezmoi actions taken out: a directive standing on
/// its own line goes with the line, and an action inside a value becomes
/// the string the vault would have put there.
///
/// NOT A CHEZMOI, and it does not need to be. What this test reads is which
/// KEYS the file names and under which tables, and no action in it is a key
/// or a table; they are one conditional wrapper and five secrets.
fn rendered_template() -> String {
    super::strip_chezmoi_actions(SHIPPED_TEMPLATE, super::identity_placeholder)
        .expect("the shipped template's own actions are well-formed")
}

#[path = "snapshot_tests.rs"]
mod snapshot;
#[path = "template_tests.rs"]
mod template;
#[path = "wording_tests.rs"]
mod wording;
