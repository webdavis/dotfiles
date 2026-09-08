use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
use crate::lanes::stubs::ScriptedRunner;
use crate::runner::SystemRunner;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
fn config() -> SkillsConfig {
    SkillsConfig {
        lock: "/lock".into(),
        agents: "/agents".into(),
        claude_skills: "/claude".into(),
        hermes: "/hermes".into(),
        npx: "/fixture/npx".into(),
        skills_cli_version: "1.5.22".into(),
        clawhub: "/fixture/clawhub".into(),
        hermes_cli: "/hermes-cli".into(),
        cua_driver: "/cua".into(),
        routing: "/routing".into(),
    }
}
fn setup() -> (
    std::path::PathBuf,
    SkillsCandidate,
    SkillsRoster,
    SkillsEnvironment,
) {
    let root = directory();
    let c = SkillsCandidate {
        home: root.join("candidate"),
    };
    std::fs::create_dir_all(c.agents()).unwrap();
    let p = root.join("roster");
    write_roster(&p, &roster());
    let roster = SkillsRoster::read(&p).unwrap();
    let env = SkillsEnvironment::for_candidate(&root.join("real"), &c).unwrap();
    (root, c, roster, env)
}
#[test]
fn the_npx_command_is_run_once_per_repo_group_with_every_skill_of_that_group() {
    let (_, c, roster, env) = setup();
    let runner = ScriptedRunner::new(&[]);
    c.install_npx(&config(), &roster, SkillsBuildMode::Full, &env, &runner)
        .unwrap();
    assert_eq!(
        runner.calls(),
        vec![vec![
            "/fixture/npx",
            "--yes",
            "skills@1.5.22",
            "add",
            "owner/repo",
            "--skill",
            "alpha",
            "--skill",
            "beta",
            "--agent",
            "claude-code",
            "--agent",
            "codex",
            "-g",
            "-y"
        ]]
    );
    assert_eq!(runner.environments(), vec![env.variables]);
}
#[test]
fn the_child_environment_keeps_candidate_roots_and_only_the_explicit_interpreter_path() {
    let (root, c, _, env) = setup();
    let expected = BTreeMap::from([
        ("HOME", c.home.clone()),
        ("XDG_CACHE_HOME", c.home.join(".cache")),
        ("XDG_CONFIG_HOME", c.home.join(".config")),
        ("XDG_DATA_HOME", c.home.join(".local/share")),
        ("XDG_STATE_HOME", c.home.join(".local/state")),
        ("TMPDIR", c.home.join(".tmp")),
        ("npm_config_cache", c.home.join(".npm")),
        (
            "CLAWHUB_CONFIG_PATH",
            c.home.join(".config/clawhub/config.json"),
        ),
    ]);
    for (key, path) in expected {
        assert_eq!(
            env.variables.get(key),
            Some(&path.to_string_lossy().into_owned()),
            "{key}"
        );
    }
    assert_eq!(
        env.variables["PATH"],
        format!(
            "{}/.local/share/fnm/aliases/default/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            root.join("real").display()
        )
    );
    assert_eq!(env.variables.len(), 9);
}
#[test]
fn an_env_node_child_starts_with_the_explicit_path_and_fails_without_it() {
    let (root, c, _, mut env) = setup();
    let bin = root.join("real/.local/share/fnm/aliases/default/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let node = bin.join("node");
    std::fs::write(
        &node,
        r#"#!/bin/sh
printf '%s\n' "$HOME" "$npm_config_cache" "${GIT_CONFIG_GLOBAL-unset}"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o700)).unwrap();
    let installer = root.join("installer");
    std::fs::write(
        &installer,
        "#!/usr/bin/env node
",
    )
    .unwrap();
    std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o700)).unwrap();
    let runner = SystemRunner::for_lane(
        "owned-installer",
        Duration::from_millis(500),
        Duration::from_millis(500),
    );
    assert_eq!(
        runner
            .run_in(installer.to_str().unwrap(), &[], &env.variables)
            .unwrap(),
        format!(
            "{}
{}
unset
",
            c.home.display(),
            c.home.join(".npm").display()
        )
    );
    env.variables.remove("PATH");
    assert!(
        runner
            .run_in(installer.to_str().unwrap(), &[], &env.variables)
            .is_err()
    );
}
#[test]
fn a_skill_the_cli_failed_is_named_and_the_others_proceed() {
    let (_, c, mut roster, env) = setup();
    roster.npx.insert("delta".into(), "z/other".into());
    let call = [
        "/fixture/npx",
        "--yes",
        "skills@1.5.22",
        "add",
        "owner/repo",
        "--skill",
        "alpha",
        "--skill",
        "beta",
        "--agent",
        "claude-code",
        "--agent",
        "codex",
        "-g",
        "-y",
    ];
    let runner = ScriptedRunner::new(&[&call]);
    let failures = c
        .install_npx(&config(), &roster, SkillsBuildMode::Full, &env, &runner)
        .unwrap();
    assert_eq!(
        failures.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["alpha", "beta"]
    );
    assert_eq!(runner.calls().len(), 2);
    assert_eq!(runner.calls()[1][4], "z/other");
}
#[test]
fn a_candidate_lock_holding_two_documents_is_refused() {
    let (_, c, roster, env) = setup();
    std::fs::write(c.agents().join(".skill-lock.json"), "{} {}").unwrap();
    let error = c
        .install_npx(
            &config(),
            &roster,
            SkillsBuildMode::Full,
            &env,
            &ScriptedRunner::new(&[]),
        )
        .unwrap_err();
    assert!(error.contains("lock"), "{error}");
}
