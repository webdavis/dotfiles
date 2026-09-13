use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
use crate::lanes::skills::{SkillsCandidate, SkillsRoster};
fn setup() -> (SkillsCandidate, SkillsRoster) {
    let root = directory();
    let c = SkillsCandidate {
        home: root.join("home"),
    };
    for name in ["alpha", "beta"] {
        std::fs::create_dir_all(c.agents().join("skills").join(name).join("agents")).unwrap();
    }
    let path = root.join("roster");
    write_roster(&path, &roster());
    (c, SkillsRoster::read(&path).unwrap())
}
#[test]
fn an_on_demand_skill_gets_the_codex_overlay_and_a_core_one_does_not() {
    let (c, r) = setup();
    let alpha = c.agents().join("skills/alpha/agents/openai.yaml");
    let beta = c.agents().join("skills/beta/agents/openai.yaml");
    std::fs::write(&alpha, "interface: upstream\n").unwrap();
    std::fs::write(&beta, format!("interface: core\n{POLICY}")).unwrap();
    c.assert_overlays(&r).unwrap();
    let overlaid = std::fs::read_to_string(&alpha).unwrap();
    let actual: serde_yaml_ng::Value = serde_yaml_ng::from_str(&overlaid).unwrap();
    assert_eq!(actual["interface"], "upstream");
    assert_eq!(actual["policy"]["allow_implicit_invocation"], false);
    assert_eq!(std::fs::read_to_string(&beta).unwrap(), "interface: core\n");
    c.assert_overlays(&r).unwrap();
    assert_eq!(std::fs::read_to_string(alpha).unwrap(), overlaid);
}
#[test]
fn an_overlay_never_writes_through_a_foreign_symlink() {
    let (c, r) = setup();
    let outside = directory().join("metadata.yaml");
    std::fs::write(&outside, "foreign content\n").unwrap();
    std::os::unix::fs::symlink(&outside, c.agents().join("skills/alpha/agents/openai.yaml"))
        .unwrap();
    assert!(c.assert_overlays(&r).unwrap_err().contains("symlink"));
    assert_eq!(
        std::fs::read_to_string(outside).unwrap(),
        "foreign content\n"
    );
}

#[test]
fn an_existing_policy_is_overridden_without_losing_metadata_and_strips_byte_exactly() {
    let (c, r) = setup();
    let path = c.agents().join("skills/alpha/agents/openai.yaml");
    let original = "# upstream formatting\ninterface: {display_name: 'Review'}\npolicy:\n  allow_implicit_invocation: true\n  tools: [read, write]\ndependencies:\n  tools: [{type: mcp, value: example}]\n";
    std::fs::write(&path, original).unwrap();
    c.assert_overlays(&r).unwrap();
    let overlaid = std::fs::read_to_string(&path).unwrap();
    let actual: serde_yaml_ng::Value = serde_yaml_ng::from_str(&overlaid).unwrap();
    let mut expected: serde_yaml_ng::Value = serde_yaml_ng::from_str(original).unwrap();
    expected["policy"]["allow_implicit_invocation"] = false.into();
    assert_eq!(actual, expected);
    c.assert_overlays(&r).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), overlaid);
    strip_owned(&path).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[test]
fn duplicated_legacy_overlays_are_repaired_and_restore_the_upstream_policy() {
    let (c, r) = setup();
    let path = c.agents().join("skills/alpha/agents/openai.yaml");
    let upstream = "interface: {display_name: Review}\npolicy:\n  allow_implicit_invocation: true\n  tools: [read]\n";
    std::fs::write(&path, format!("{upstream}{POLICY}{POLICY}")).unwrap();
    c.assert_overlays(&r).unwrap();
    let actual: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(actual["policy"]["allow_implicit_invocation"], false);
    assert_eq!(actual["policy"]["tools"][0], "read");
    strip_owned(&path).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), upstream);
}

#[test]
fn malformed_upstream_metadata_is_refused_without_writing() {
    for original in [
        "interface: [broken",
        "interface: first\ninterface: second\n",
        "policy: false\n",
        "policy: {allow_implicit_invocation: true, allow_implicit_invocation: false}\n",
        "- a sequence\n",
    ] {
        let (c, r) = setup();
        let path = c.agents().join("skills/alpha/agents/openai.yaml");
        std::fs::write(&path, original).unwrap();
        assert!(c.assert_overlays(&r).is_err(), "accepted {original}");
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
}

#[test]
fn changing_to_core_restores_upstream_metadata_including_its_invocation_setting() {
    for original in [
        "policy: {allow_implicit_invocation: true, other: retained}\n",
        "policy: {allow_implicit_invocation: false, other: retained}\n",
        "interface: {display_name: 'Review'}\n",
        " \n",
    ] {
        let (c, mut r) = setup();
        let path = c.agents().join("skills/alpha/agents/openai.yaml");
        std::fs::write(&path, original).unwrap();
        c.assert_overlays(&r).unwrap();
        r.tiers.insert("alpha".into(), "core".into());
        c.assert_overlays(&r).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        c.assert_overlays(&r).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
}

#[test]
fn stripping_an_owned_override_refuses_to_erase_subsequent_metadata_edits() {
    let (c, r) = setup();
    let path = c.agents().join("skills/alpha/agents/openai.yaml");
    std::fs::write(
        &path,
        "interface: original\npolicy: {allow_implicit_invocation: true}\n",
    )
    .unwrap();
    c.assert_overlays(&r).unwrap();
    let edited = std::fs::read_to_string(&path).unwrap().replacen(
        "interface: original",
        "interface: edited",
        1,
    );
    std::fs::write(&path, &edited).unwrap();
    assert!(strip_owned(&path).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), edited);
}

#[test]
fn overriding_an_inherited_policy_preserves_its_other_fields_and_original_bytes() {
    let (c, r) = setup();
    let path = c.agents().join("skills/alpha/agents/openai.yaml");
    let original = "defaults: &defaults\n  policy: {allow_implicit_invocation: true, tools: [read]}\n<<: *defaults\ninterface: {display_name: 'Révision'}\n";
    std::fs::write(&path, original).unwrap();
    c.assert_overlays(&r).unwrap();
    let mut actual: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    actual.apply_merge().unwrap();
    assert_eq!(actual["policy"]["allow_implicit_invocation"], false);
    assert_eq!(actual["policy"]["tools"][0], "read");
    assert_eq!(actual["interface"]["display_name"], "Révision");
    strip_owned(&path).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}
