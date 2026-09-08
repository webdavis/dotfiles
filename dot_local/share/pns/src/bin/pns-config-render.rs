//! Dev-only: turns the committed values file into the shipped chezmoi
//! template. Never installed (see the build script under
//! `.chezmoiscripts`, which only ever copies `target/release/pns`); run by
//! hand through `just pns-config-render`.
//!
//! THE FLOW IS READ, REFUSE-OR-RENDER, SELF-PARSE, WRITE, in that order, and
//! nothing is written to the template path until every earlier step
//! succeeded: a values file that renders something the parser itself would
//! reject must never reach disk, because the shipped template is the config
//! of record and a rejected file falls the machine back to the CORE alone.
//!
//! THE WRAPPER (the banner and the `{{- if eq .chezmoi.os "darwin" }}` /
//! `{{- end }}` pair) is added HERE, not inside `config_text::render`: that
//! function's other caller is the first-run wizard, which writes a real file
//! straight to disk with no chezmoi templating step at all, and a literal
//! `{{- if }}` line in that file would never be resolved.

use std::process::ExitCode;

use pns::config::{Config, identity_placeholder, parse_config, strip_chezmoi_actions};
use pns::config_text::render;

/// The banner is independently written in the binary acceptance test.
const BANNER: &str = "\
# GENERATED FILE: this is `render`'s own text over the committed
# `dot_config/pns/config-values.toml`, produced by `just pns-config-render`.
# EDIT THE VALUES FILE AND REGENERATE; a hand edit here fails this test.
{{- if eq .chezmoi.os \"darwin\" }}

";
const FOOTER: &str = "{{- end }}\n";

/// The five keys this repo's own values file treats as secret-bearing:
/// present, each must hold a keepassxc marker table rather than a literal.
///
/// A SCAN OF THE RENDERED TEXT CANNOT STAND IN FOR THIS: `render` accepts a
/// plain string for any of these keys just as happily as it accepts a
/// marker table (the schema does not know these five are special), so only
/// a check that reads the VALUES FILE ITSELF, before it is rendered, can
/// catch a pasted credential landing in the file this repo commits.
const SECRET_BEARING_KEYS: [&str; 5] = [
    "plugins.mobile.token",
    "plugins.hermes.key",
    "plugins.hue.bridge",
    "plugins.hue.key",
    "plugins.router.api_key",
];

const RESOLVED_CONFIG_SNAPSHOT: &str =
    include_str!("../../tests/fixtures/resolved-config.snapshot");

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [mode, values_path] if mode == "--check" => check(values_path),
        [values_path, template_path] => run(values_path, template_path).map(|()| {
            println!("wrote {template_path}");
        }),
        _ => {
            eprintln!(
                "usage: pns-config-render <values-file> <template-file>\n       pns-config-render --check <values-file>"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pns-config-render: refused: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(values_path: &str, template_path: &str) -> Result<(), String> {
    let (rendered, _) = rendered_configuration(values_path)?;
    let template = format!("{BANNER}{rendered}{FOOTER}");
    std::fs::write(template_path, template)
        .map_err(|error| format!("writing {template_path}: {error}"))
}

fn check(values_path: &str) -> Result<(), String> {
    let (_, config) = rendered_configuration(values_path)?;
    pns::registry::roster()
        .enabled(&config.plugin_switches())
        .map_err(|error| format!("the render names an unregistered plugin: {error:?}"))?;
    if format!("{config:#?}\n") != RESOLVED_CONFIG_SNAPSHOT {
        return Err("resolved configuration differs from the committed snapshot".into());
    }
    Ok(())
}

fn rendered_configuration(values_path: &str) -> Result<(String, Config), String> {
    let values_text = std::fs::read_to_string(values_path)
        .map_err(|error| format!("reading {values_path}: {error}"))?;
    let values: toml::Table = values_text
        .parse()
        .map_err(|error: toml::de::Error| format!("{values_path} is not valid TOML: {error}"))?;

    refuse_literal_secrets(&values)?;

    let rendered = render(&values).map_err(|error| format!("rendering {values_path}: {error}"))?;

    // SELF-PARSE BEFORE WRITING ANYTHING: a chezmoi secret action is not
    // valid TOML on its own (`{{` is not a TOML token), so the same stub the
    // crate's own template tests use stands in for chezmoi's substitution
    // first.
    let stubbed = strip_chezmoi_actions(&rendered, identity_placeholder)
        .map_err(|error| format!("the render carries a malformed secret action: {error}"))?;
    let config = parse_config(&stubbed)
        .map_err(|error| format!("the render does not self-parse: {}", error.detail()))?;
    Ok((rendered, config))
}

/// Refuses by name when one of `SECRET_BEARING_KEYS` is present but is not a
/// table: `render` itself validates a present table's shape (the entry name,
/// the field), so this only has to rule out a literal standing in its place.
fn refuse_literal_secrets(values: &toml::Table) -> Result<(), String> {
    for path in SECRET_BEARING_KEYS {
        if let Some(value) = lookup(values, path)
            && !matches!(value, toml::Value::Table(_))
        {
            return Err(format!(
                "`{path}` must be a keepassxc secret marker table, not a literal value"
            ));
        }
    }
    Ok(())
}

/// A dotted path into a values table, stopping at the first segment that is
/// missing or is not itself a table.
fn lookup<'a>(table: &'a toml::Table, dotted: &str) -> Option<&'a toml::Value> {
    let mut current = table;
    let mut segments = dotted.split('.').peekable();
    while let Some(segment) = segments.next() {
        let value = current.get(segment)?;
        if segments.peek().is_none() {
            return Some(value);
        }
        current = value.as_table()?;
    }
    None
}

#[cfg(test)]
#[path = "pns-config-render/tests.rs"]
mod tests;
