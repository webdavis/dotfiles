use super::*;

#[test]
fn the_committed_template_is_render_over_the_committed_values_file() {
    // THE WRAPPER TEXT IS DUPLICATED BY HAND rather than imported from
    // `pns-config-render`, on purpose: if that binary's own copy were
    // ever deleted or gutted to an empty string, importing it here would
    // make both sides agree on nothing and this test would still pass.
    // A hand-kept second copy is what turns that mutant red.
    const BANNER: &str = "\
# GENERATED FILE: this is `render`'s own text over the committed
# `dot_config/pns/config-values.toml`, produced by `just pns-config-render`.
# EDIT THE VALUES FILE AND REGENERATE; a hand edit here fails this test.
{{- if eq .chezmoi.os \"darwin\" }}

";
    const FOOTER: &str = "{{- end }}\n";

    let values: toml::Table = CONFIG_VALUES
        .parse()
        .expect("the committed values file is valid TOML");
    let rendered = crate::config_text::render(&values).expect("the committed values file renders");
    let expected = format!("{BANNER}{rendered}{FOOTER}");
    assert_eq!(
        expected, SHIPPED_TEMPLATE,
        "the shipped template drifted from `render` over the committed values file; \
             regenerate with `just pns-config-render`"
    );
}

#[test]
fn every_table_the_operator_runs_is_still_live_in_the_shipped_template() {
    // A COMMENTED heading (`# [nag]`) does not start with `[` once
    // trimmed, which is exactly the difference being measured here.
    let live: Vec<&str> = SHIPPED_TEMPLATE
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
        })
        .collect();
    assert_eq!(
        live, LIVE_TABLES,
        "a table the operator runs is no longer live in the shipped template; a table \
             missing from `dot_config/pns/config-values.toml` renders COMMENTED OUT, so check \
             that file before changing this list"
    );
}

#[test]
fn the_stub_refuses_a_secret_action_that_forgot_totoml() {
    // THE MUTANT THIS PINS: a template secret line with `| toToml`
    // dropped. Chezmoi would then splice the raw vault bytes in unquoted
    // and the deployed file would not parse, but a stub that swaps ANY
    // action for a quoted placeholder would keep every template test
    // green. So the stub only stands in for the one action grammar the
    // renderer writes, and refuses the rest out loud.
    let error = super::strip_chezmoi_actions(
        "token = {{ (keepassxc \"Moshi :: Webhook Secret\").Password }}",
        |_, _| "\"from-the-vault\"".to_string(),
    )
    .expect_err("a bare action with no `| toToml` is not a secret action");
    assert!(error.contains("not a `| toToml` secret action"), "{error}");
}

/// THE STUB ONLY READS THE GRAMMAR of a secret action, which is what
/// keeps a dropped `| toToml` or an unknown field from passing itself off
/// as vault output. It reads neither WHICH entry a line names nor WHICH
/// field it takes off that entry, so pointing hue's `bridge` at
/// `.Password` or a line at another vault entry leaves every other
/// template test green while the deployed file quietly carries the wrong
/// credential, and both are one character.
///
/// NOTHING RENDERS THIS TEMPLATE IN A TEST, so its own text is the only
/// place that agreement can sit until PR S2 generates the file from
/// `config_text::render` and compares the two byte for byte. The list is
/// exact rather than a `contains` per line, so a sixth secret appearing,
/// or one of these five going away, is the same red.
///
/// EACH LINE CARRIES ITS OWN TABLE, not just its text: a bare line
/// comparison cannot tell hermes's secret sitting under `[plugins.hue]`
/// from hermes's secret sitting under `[plugins.hermes]`, since the line
/// text alone never says which heading it fell under (sol-1 finding 1).
#[test]
fn the_shipped_template_names_the_entry_and_field_of_every_secret() {
    let mut table = String::new();
    let secrets: Vec<(String, &str)> = SHIPPED_TEMPLATE
        .lines()
        .filter_map(|line| {
            if let Some(heading) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                table = heading.to_string();
                return None;
            }
            line.contains("keepassxc").then(|| (table.clone(), line))
        })
        .collect();
    assert_eq!(
        secrets,
        [
            (
                "plugins.mobile".to_string(),
                r#"token = {{ (keepassxc "Moshi :: Webhook Secret").Password | toToml }}"#
            ),
            (
                "plugins.hermes".to_string(),
                r#"key = {{ (keepassxc "Hermes :: Webhook Secret :: #pns").Password | toToml }}"#
            ),
            (
                "plugins.hue".to_string(),
                r#"bridge = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").UserName | toToml }}"#
            ),
            (
                "plugins.hue".to_string(),
                r#"key = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").Password | toToml }}"#
            ),
            (
                "plugins.router".to_string(),
                r#"api_key = {{ (keepassxc "UniFi :: API Key (dresden-udr)").Password | toToml }}"#
            ),
        ]
    );
}

#[test]
fn the_shipped_config_template_still_parses_through_this_schema() {
    // THE FENCE UNDER THE SWEEP. Judging every plugin table's keys can
    // refuse a config that worked yesterday, and the only config that
    // matters is the one this repo ships. If it stops loading, the
    // machine falls back to the CORE with a warning nobody is standing in
    // front of: the phone and the banner keep working, and the durable
    // paper trail, the lights and the home probe all stop.
    let rendered = rendered_template();
    let config = parse_config(&rendered)
        .unwrap_or_else(|error| panic!("the shipped template must load: {error:?}"));
    assert_eq!(
        config.plugins.keys().collect::<Vec<_>>(),
        vec![
            "hermes",
            "hue",
            "macos-banner",
            "mobile",
            "presence",
            "router"
        ],
        "and it must still select what it selects"
    );
    // And every one of those names is a plugin that exists, which is the
    // refusal one layer on.
    crate::registry::roster()
        .enabled(&config.plugin_switches())
        .expect("the template names only registered plugins");
}

#[test]
fn the_shipped_template_states_the_blocked_backstop_at_its_default_uncommented() {
    // DEFAULTS VISIBLE IN CONFIG (operator ruling): the key fence counts a
    // commented line too, and the parser reads the same number whether the
    // line is there or not, so only the line itself pins the ruling.
    assert!(
        rendered_template()
            .lines()
            .any(|line| line == "give_up_after_secs = 57600"),
        "the template must state the blocked backstop, uncommented, at 57600"
    );
}
