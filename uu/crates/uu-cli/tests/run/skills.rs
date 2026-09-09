#[path = "skills/fixture.rs"]
mod fixture;
use super::*;
use fixture::*;
#[test]
fn a_registered_skills_lane_runs_weekly_under_its_declared_name() {
    let f = Fixture::new("skills-weekly");
    f.ready("ready");
    let output = f.invoke(&["run", "mine"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("mine: 0 failure(s)"), "{output:?}");
    assert!(
        stdout(&output).contains("reusing recovered candidate"),
        "{output:?}"
    );
    assert!(f.published().contains("ready"));
    assert!(f.home.dir.join("routing-called").is_file());
}
#[test]
fn a_registered_skills_bootstrap_fills_delivery_without_a_weekly_record() {
    let f = Fixture::new("skills-bootstrap");
    let output = f.invoke(&["bootstrap", "mine"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("healthy; no publication"),
        "{output:?}"
    );
    assert!(f.home.dir.join(".hermes/skills/alpha/SKILL.md").is_file());
    assert!(!f.home.marker().exists());
    assert!(f.home.dir.join("routing-called").is_file());
}
#[test]
fn a_registered_skills_bootstrap_propagates_required_phase_failure() {
    let f = Fixture::new("skills-bootstrap-failure");
    f.home
        .write_stub("routing", "printf 'owned routing refusal\n' >&2\nexit 1\n");
    let output = f.invoke(&["bootstrap", "mine"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stdout(&output).contains("owned routing refusal"),
        "{output:?}"
    );
    assert!(!f.home.marker().exists());
}
#[test]
fn updater_identity_is_captured_before_an_earlier_lane_replaces_the_running_path() {
    let f = Fixture::new("skills-startup-identity");
    f.ready("ready");
    let replace = f.home.write_stub(
        "replace-binary",
        "printf replacement >\"$HOME/replacement\"\nmv \"$HOME/replacement\" \"$HOME/uu-copy\"\n",
    );
    let config = f.home.dir.join(".config/uu/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "\n[lanes.a-before]\ntype = \"command\"\nrun = [{replace:?}]\n"
    ));
    std::fs::write(config, text).unwrap();
    let output = f.invoke(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(std::fs::read_to_string(&f.binary).unwrap(), "replacement");
    assert!(
        stdout(&output).contains("reusing recovered candidate"),
        "{output:?}"
    );
    assert!(f.published().contains("ready"));
    assert!(!f.home.dir.join("installer-called").exists());
}
