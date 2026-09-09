use super::*;
use crate::lanes::skills::tests::{config, directory};
use crate::lanes::stubs::ScriptedRunner;
#[test]
fn a_failed_pack_refresh_is_a_counted_failure_naming_the_app() {
    let root = directory();
    let config = config(&root);
    let store = root.join(".agents/skills");
    std::fs::create_dir_all(&store).unwrap();
    std::os::unix::fs::symlink(root.join("app-owned"), store.join("cua-driver")).unwrap();
    let runner = ScriptedRunner::new(&[&[&config.cua_driver, "skills", "update"]])
        .answering("pack failure detail");
    let mut report = LaneReport::new("skills");
    config.refresh_app_pack(&runner, &mut report);
    assert_eq!(report.failures(), 1);
    assert!(
        report.lines[0].contains("cua-driver") && report.lines[0].contains("pack failure detail")
    );
    assert_eq!(
        runner.calls(),
        vec![vec![config.cua_driver, "skills".into(), "update".into()]]
    );
    assert_eq!(
        std::fs::read_link(store.join("cua-driver")).unwrap(),
        root.join("app-owned")
    );
}
#[test]
fn a_routing_assertion_that_fails_is_a_counted_failure_carrying_its_output() {
    let root = directory();
    let config = config(&root);
    let runner = ScriptedRunner::new(&[&[&config.routing, "--check"], &[&config.routing]])
        .answering("stale mirror detail");
    let mut report = LaneReport::new("skills");
    config.assert_routing(&runner, &mut report);
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("routing") && s.contains("stale mirror detail"))
    );
    assert_eq!(
        runner.calls(),
        vec![
            vec![config.routing.clone(), "--check".into()],
            vec![config.routing]
        ]
    );
}
#[test]
fn clean_routing_is_not_rewritten_and_an_absent_app_pack_is_not_refreshed() {
    let root = directory();
    let config = config(&root);
    let runner = ScriptedRunner::new(&[]).answering("routing clean detail");
    let mut report = LaneReport::new("skills");
    config.refresh_app_pack(&runner, &mut report);
    config.assert_routing(&runner, &mut report);
    assert_eq!(runner.calls(), vec![vec![config.routing, "--check".into()]]);
    assert_eq!(report.failures(), 0);
    assert!(
        report
            .lines
            .iter()
            .any(|s| s.contains("routing clean detail"))
    );
}
