use super::*;
use std::process::{Command, Stdio};
use std::time::Instant;

const CASE: &str = "lanes::nvim::smoke_test::tests::isolation::the_real_smoke_children_keep_external_home_and_discovery_unchanged";
const ROOT: &str = "UU_SMOKE_ISOLATION_ROOT";

fn snapshot(path: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(snapshot(&path));
        } else {
            result.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    result.sort();
    result
}

#[test]
fn the_real_smoke_children_keep_external_home_and_discovery_unchanged() {
    if let Some(root) = std::env::var_os(ROOT) {
        let root = PathBuf::from(root);
        let lane = NvimSmokeTestLane {
            host: NvimHost {
                nvim: root.join("nvim").to_str().unwrap().into(),
                config: root.join("config").to_str().unwrap().into(),
            },
            cache: root.join("k").to_str().unwrap().into(),
        };
        let runner = crate::runner::SystemRunner::for_lane(
            "isolation",
            Duration::from_millis(600),
            Duration::from_millis(600),
        );
        let report = lane.run("isolation", &crate::lanes::stubs::stub_facts(), &runner);
        assert_eq!(
            report.verdict(),
            LaneVerdict::Completed,
            "{:?}",
            report.lines
        );
        return;
    }
    let f = Fixture::new("real-isolation");
    let root = f.cache.parent().unwrap();
    let external_home = root.join("external-home");
    let external_claude = root.join("external-claude");
    for path in [&external_home, &external_claude] {
        fs::create_dir(path).unwrap();
        fs::write(path.join("sentinel"), b"preserve exact bytes").unwrap();
    }
    let before_home = snapshot(&external_home);
    let before_claude = snapshot(&external_claude);
    let launcher = root.join("nvim");
    fs::write(&launcher, r#"#!/bin/sh
set -eu
phase=verify
for arg do
  if [ "$arg" = prepare ]; then phase=prepare; fi
done
printf 'owned home state' > "$HOME/home-$phase"
mkdir -p "$CLAUDE_CONFIG_DIR/ide"
printf 'owned discovery' > "$CLAUDE_CONFIG_DIR/ide/discovery-$phase"
printf 'ready' > "$UU_SMOKE_ISOLATION_ROOT/$phase-ready"
while [ ! -f "$UU_SMOKE_ISOLATION_ROOT/$phase-ack" ]; do sleep 0.002; done
if [ "$phase" = prepare ]; then
  printf 'owned candidate lock' > "$XDG_CONFIG_HOME/nvim/lazy-lock.json"
else
  jq -n --slurpfile request "$UU_SMOKE_ISOLATION_ROOT/k/run.json" --rawfile lock "$XDG_CONFIG_HOME/nvim/lazy-lock.json" '{run:$request[0].run,lock:$lock,vim_enter:true,errors:[],health_errors:0,health_warnings:0}' > "$UU_SMOKE_ISOLATION_ROOT/k/completion.json"
fi
mv "$CLAUDE_CONFIG_DIR/ide/discovery-$phase" "$CLAUDE_CONFIG_DIR/ide/discovery-$phase.closed"
"#).unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CASE, "--nocapture"])
        .env(ROOT, root)
        .env("HOME", &external_home)
        .env("CLAUDE_CONFIG_DIR", &external_claude)
        .env("XDG_DATA_HOME", &f.data)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut observations = Vec::new();
    for phase in ["prepare", "verify"] {
        while !root.join(format!("{phase}-ready")).exists()
            && start.elapsed() < Duration::from_millis(750)
        {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        observations.push((
            phase,
            f.cache.join(format!("h/home-{phase}")).exists(),
            f.cache
                .join(format!("h/.claude/ide/discovery-{phase}"))
                .exists(),
            snapshot(&external_home) == before_home,
            snapshot(&external_claude) == before_claude,
        ));
        fs::write(root.join(format!("{phase}-ack")), b"checked").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        observations
            .iter()
            .all(|(_, h, c, eh, ec)| *h && *c && *eh && *ec),
        "home/discovery isolation failed during pause: {observations:?}"
    );
    assert_eq!(
        snapshot(&external_home),
        before_home,
        "external HOME changed after shutdown"
    );
    assert_eq!(
        snapshot(&external_claude),
        before_claude,
        "external Claude discovery changed after cleanup"
    );
    for phase in ["prepare", "verify"] {
        assert!(
            f.cache
                .join(format!("h/.claude/ide/discovery-{phase}.closed"))
                .exists()
        );
    }
}
