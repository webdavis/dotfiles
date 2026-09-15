use posture_domain::FUNNEL_EXPOSURE_KEY_LIMIT;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn compare(name: &str) {
    let started = Instant::now();
    let cases: Vec<Value> = serde_json::from_str(include_str!("captures.json")).unwrap();
    let mut case = cases
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone();
    if name == "oversized_exposure" {
        // Bash delivered every exposed key in one page past the wire cap. The captured
        // key lines are replayed up to the domain key limit and the rest become one
        // summary line; the exit code, the baseline advance and the empty stderr are
        // the capture's own.
        let body = case["expected"]["alerts"][0]["body"].as_str().unwrap();
        assert!(body.chars().count() > 8000);
        let mut lines: Vec<String> = body.lines().map(str::to_owned).collect();
        let header = lines
            .iter()
            .position(|line| line.starts_with("- `"))
            .unwrap();
        let omitted = lines.split_off(header + FUNNEL_EXPOSURE_KEY_LIMIT).len();
        lines.push(format!("- …and {omitted} more"));
        case["expected"]["alerts"][0]["body"] = lines.join("\n").into();
    }
    let home = std::env::temp_dir().join(format!("posture-funnel-{}-{name}", std::process::id()));
    fs::create_dir(&home).unwrap();
    let home = home.canonicalize().unwrap();
    let state = home.join("state/baseline");
    fs::create_dir(state.parent().unwrap()).unwrap();
    if let Some(prior) = case["prior"].as_str() {
        fs::write(&state, prior).unwrap();
    }
    for (key, suffix) in [("gap", ".gap"), ("persist_gap", ".persist-gap")] {
        if case[key] == true {
            fs::write(home.join(format!("state/baseline{suffix}")), "").unwrap();
        }
    }
    if case["fail_publish"] == true {
        fs::create_dir(home.join("state/baseline.tmp")).unwrap();
    }
    let tailscale = home.join("tailscale");
    if case["missing"] != true {
        executable(
            &tailscale,
            &format!(
                "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$*\" >\"$HOME/argv\"\n/bin/cat \"$HOME/input\"\nexit {}\n",
                case["exit"]
            ),
        );
    }
    if case["sleep"] == true {
        executable(&tailscale, "#!/bin/sh\nexec /bin/sleep 2\n");
    }
    fs::write(home.join("input"), case["input"].as_str().unwrap()).unwrap();
    let engine = home.join(".local/libexec/engine");
    fs::create_dir_all(engine.parent().unwrap()).unwrap();
    // Point posture's delivery at this owned fixture command, the way the
    // config deployed on a real machine points it at that machine's engine.
    let delivery = home.join(".config/posture/config.toml");
    fs::create_dir_all(delivery.parent().unwrap()).unwrap();
    fs::write(
        &delivery,
        format!(
            "[delivery]\nmode = \"producer\"\n[delivery.producer]\ncommand = \"{}\"\narguments = [\"submit\", \"--json\"]\n",
            engine.display()
        ),
    )
    .unwrap();
    let (status, diagnostics, exit) = if case["reject"] == true {
        ("rejected", "", 2)
    } else {
        ("accepted", "\"ledger_committed\"", 0)
    };
    executable(
        &engine,
        &format!(
            r##"#!/bin/sh
set -eu
[ "$#" = 2 ] && [ "$1" = submit ] && [ "$2" = --json ] || exit 42
IFS= read -r request
printf '%s\n' "$request" >>"$HOME/requests"
n=$(/usr/bin/wc -l <"$HOME/requests" | /usr/bin/tr -d ' ')
if [ -f "$OSQUERY_TAILSCALE_STATE" ]; then /bin/cp "$OSQUERY_TAILSCALE_STATE" "$HOME/prior-$n"; fi
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{{"schema":"pns.result/1","request_id":"%s","status":"{status}","diagnostics":[{diagnostics}]}}\n' "$identity"
exit {exit}
"##
        ),
    );
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command.args(["-p", &format!("(version 1) (allow default) (deny network*) (deny process-exec (literal \"/usr/bin/osascript\")) (deny file-write* (require-all (require-not (subpath \"{}\")) (require-not (literal \"/dev/null\"))))", home.display())]);
        command.arg(env!("CARGO_BIN_EXE_posture"));
        command
    } else {
        Command::new(env!("CARGO_BIN_EXE_posture"))
    };
    let mut child = command
        .env_clear()
        .env("HOME", &home)
        .env("TMPDIR", &home)
        .env("PATH", "/usr/bin:/bin")
        .env("OSQUERY_TAILSCALE_STATE", &state)
        .env("OSQUERY_TAILSCALE_BIN", &tailscale)
        .env(
            "OSQUERY_TAILSCALE_TIMEOUT",
            case["timeout"]
                .as_str()
                .unwrap_or(if case["sleep"] == true { "0.02" } else { "0.5" }),
        )
        .args(["funnel", "ignored operand"])
        .current_dir(&home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_millis(900) => {
                std::thread::sleep(Duration::from_millis(1))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned funnel deadline: {result:?}");
            }
        }
    }
    let output = child.wait_with_output().unwrap();
    if case["missing"] != true && case["sleep"] != true {
        assert_eq!(
            fs::read_to_string(home.join("argv")).ok().as_deref(),
            Some("funnel status --json\n"),
            "{name}: status inspection must run"
        );
    }
    let expected = &case["expected"];
    assert_eq!(
        output.status.code(),
        Some(expected["code"].as_i64().unwrap() as i32),
        "{name}: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    if case["fail_publish"] != true {
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).replace(home.to_str().unwrap(), "<HOME>"),
            expected["stderr"]
                .as_str()
                .unwrap()
                .split_inclusive('\n')
                .filter(|line| !line
                    .ends_with("warning: command substitution: ignored null byte in input\n"))
                .collect::<String>()
                // The capture names the retired Bash script and its send_alert
                // function. posture names the command the operator ran.
                .replace(
                    "tailscale-monitor: send_alert could not queue",
                    "posture funnel: could not queue"
                )
        );
    }
    let prior = fs::read_to_string(&state).ok();
    assert_eq!(
        prior.as_deref(),
        expected["baseline"].as_str(),
        "{name} baseline"
    );
    for (key, suffix) in [("gap", ".gap"), ("persist_gap", ".persist-gap")] {
        assert_eq!(
            home.join(format!("state/baseline{suffix}")).is_file(),
            expected[key].as_bool().unwrap(),
            "{name} {key}"
        );
    }
    let requests = fs::read_to_string(home.join("requests")).unwrap_or_default();
    let requests: Vec<Value> = requests
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let alerts = expected["alerts"].as_array().unwrap();
    assert_eq!(requests.len(), alerts.len(), "{name} submissions");
    for (index, (request, alert)) in requests.iter().zip(alerts).enumerate() {
        assert_eq!(request["producer"], "posture");
        assert_eq!(request["class"], "security");
        assert_eq!(request["route"], "posture-pages");
        assert_eq!(request["signal"]["kind"], "needs_attention");
        assert_eq!(
            request["detail"]
                .as_str()
                .unwrap()
                .replace(tailscale.to_str().unwrap(), "<TAILSCALE>"),
            format!(
                "{}\n{}",
                alert["title"].as_str().unwrap(),
                alert["body"].as_str().unwrap()
            )
        );
        assert_eq!(
            fs::read_to_string(home.join(format!("prior-{}", index + 1)))
                .ok()
                .as_deref(),
            alert["prior"].as_str(),
            "{name}: notify before persist"
        );
    }
    if prior.is_some() && expected["code"] == 0 && expected["baseline"] != case["prior"] {
        assert_eq!(
            fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    fs::write(
        home.join("test-elapsed-micros"),
        started.elapsed().as_micros().to_string(),
    )
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{name}: {:?}",
        started.elapsed()
    );
}
fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
