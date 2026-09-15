use super::*;
use posture_application::{ClockUnavailable, WallTime};
use std::os::unix::fs::PermissionsExt;

mod spool;

struct NoInspection;
impl posture_adapters::CommandRunner for NoInspection {
    fn run_completed(
        &mut self,
        _: &Path,
        _: &[&std::ffi::OsStr],
        _: posture_adapters::CommandIo<'_>,
    ) -> Result<posture_adapters::CommandOutput, posture_application::InspectionFailure> {
        Err(posture_application::InspectionFailure::Unavailable)
    }
}

struct Time;
impl Clock for Time {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Ok(WallTime {
            seconds: 10_000,
            utc_day: "1970-01-01".into(),
        })
    }
}

#[test]
fn an_integrity_page_carries_the_actual_hashes_and_upgrade_record() {
    let root = std::path::PathBuf::from(format!(
        "/private/tmp/posture-triage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut config =
        Configuration::read(|name| (name == "HOME").then(|| root.clone().into())).unwrap();
    config.pipeline_manifest = root.join("pipeline-manifest");
    config.managed_bin_manifest = root.join("managed-manifest");
    config.alarm = "/usr/bin/false".into();
    let engine = root.join(".local/libexec/engine");
    config.delivery = crate::producer_delivery(&engine);
    for path in [&engine, &config.log, &config.cursor] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    let target = root.join(".local/bin/tool");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"abc").unwrap();
    std::fs::write(
        &config.managed_bin_manifest,
        format!("{} 0600 1 {}\n", "A".repeat(64), target.display()),
    )
    .unwrap();
    let upgrade = root.join(".local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv");
    std::fs::create_dir_all(upgrade.parent().unwrap()).unwrap();
    std::fs::write(
        upgrade,
        b"9000\t1970-01-01T02:30:00Z\ntool\tchanged\t1\t2\n",
    )
    .unwrap();
    std::fs::write(&config.log, format!(
        "{{\"name\":\"file_events_recent\",\"action\":\"added\",\"counter\":4,\"columns\":{{\"category\":\"managed_bin\",\"target_path\":\"{}\",\"action\":\"UPDATED\"}}}}\n",
        target.display()
    )).unwrap();
    let request = root.join("request");
    std::fs::write(&engine, format!(r##"#!/bin/sh
set -eu
IFS= read -r request
printf '%s\n' "$request" >'{}'
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{{"schema":"pns.result/1","request_id":"%s","status":"accepted","diagnostics":["ledger_committed"]}}\n' "$identity"
"##, request.display())).unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();

    let mut stderr = Vec::new();
    assert_eq!(execute(config, Time, || NoInspection, &mut stderr), 0);
    let request = std::fs::read_to_string(request).unwrap();
    for expected in [
        // THE TIER NAMES THE ROUTE, end to end, whatever route this command
        // built its sink with: a critical finding is `priority`
        // (posture_domain::severity_route).
        "\"route\":\"priority\"",
        "aaaaaaaaaaaa",
        "ba7816bf8f01",
        "recorded upgrade: tool 1 -> 2 at 1970-01-01T02:30:00Z",
        "the name matches this file, which is not proof",
    ] {
        assert!(request.contains(expected), "missing {expected}: {request}");
    }
}
