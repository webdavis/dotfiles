use super::*;
use crate::{ConfigError, LaneAdapter, LaneRegistration};
use uu_domain::RunFacts;

#[derive(Debug)]
struct Seed(String);
impl LaneAdapter for Seed {
    fn parse(_: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        Ok(Self(fields["message"].as_str().unwrap().into()))
    }
    fn keys() -> &'static [&'static str] {
        &["type", "message"]
    }
    fn run(&self, _: &str, _: &RunFacts, _: &dyn CommandRunner) -> LaneReport {
        panic!("bootstrap must not invoke the weekly lane")
    }
    fn bootstrap_capability(&self) -> Option<&dyn BootstrapLane> {
        Some(self)
    }
}
impl BootstrapLane for Seed {
    fn bootstrap(&self, name: &str, home: &str, _: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        report.noted(format!("{} at {home}", self.0));
        report
    }
}

#[test]
fn a_parsed_bootstrap_capability_receives_its_declared_name_and_private_home() {
    let config = crate::config::parse_config(
        r#"[lanes.mine]
type = "seed"
message = "owned seed"
"#,
        &[LaneRegistration::new::<Seed>("seed")],
    )
    .unwrap();
    let mut expected = LaneReport::new("mine");
    expected.noted("owned seed at /fixture/private-home".into());
    assert_eq!(
        bootstrap_lane("/fixture/private-home", &config, "mine"),
        BootstrapOutcome::Reported(expected)
    );
}
