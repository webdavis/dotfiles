use super::*;
fn fields() -> toml::Table {
    toml::from_str(
        r#"type="skills"
lock="/roster"
agents="/agents"
claude_skills="/claude"
hermes="/hermes"
npx="/bin/npx"
skills_cli_version="1.5.22"
clawhub="/bin/clawhub"
hermes_cli="/bin/hermes"
cua_driver="/bin/cua"
routing="/bin/routing"
"#,
    )
    .unwrap()
}
#[test]
fn every_skills_path_is_required_and_absolute_and_the_cli_version_is_preserved() {
    let parsed = SkillsConfig::parse_fields("lanes.mine", fields()).unwrap();
    assert_eq!(parsed.skills_cli_version, "1.5.22");
    assert_eq!(parsed.agents, "/agents");
    for key in [
        "lock",
        "agents",
        "claude_skills",
        "hermes",
        "npx",
        "clawhub",
        "hermes_cli",
        "cua_driver",
        "routing",
    ] {
        for value in [None, Some(toml::Value::String("relative".into()))] {
            let mut f = fields();
            f.remove(key);
            if let Some(value) = value {
                f.insert(key.into(), value);
            }
            let error = format!(
                "{:?}",
                SkillsConfig::parse_fields("lanes.mine", f).unwrap_err()
            );
            assert!(error.contains(key), "{key}: {error}");
        }
    }
}
#[test]
fn an_empty_skills_cli_version_or_unknown_field_is_refused_by_name() {
    for (key, value) in [("skills_cli_version", ""), ("typo", "/path")] {
        let mut f = fields();
        f.insert(key.into(), toml::Value::String(value.into()));
        let error = format!(
            "{:?}",
            SkillsConfig::parse_fields("lanes.mine", f).unwrap_err()
        );
        assert!(error.contains(key), "{error}");
    }
}
