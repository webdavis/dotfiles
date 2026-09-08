use super::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn the_public_factory_preserves_blank_override_and_backend_refusal_before_dispatch() {
    if let Ok(scenario) = std::env::var("PNS_REGISTRY_FIXTURE_SCENARIO") {
        check_factory(&scenario);
        return;
    }
    for scenario in ["unset", "blank", "forced", "refused"] {
        let directory = fixture("mobile");
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "channel_dispatch::tests::environment::the_public_factory_preserves_blank_override_and_backend_refusal_before_dispatch",
                "--nocapture",
            ])
            .env_clear()
            .env("HOME", &directory)
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", &directory)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("PNS_REGISTRY_FIXTURE_SCENARIO", scenario)
            .env("PNS_REQUEST_ID", "hostile-inherited-id")
            .env("PNS_PRODUCER", "hostile-inherited-producer")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
            "XDG_RUNTIME_DIR",
            "XDG_CONFIG_DIRS",
            "XDG_DATA_DIRS",
            "CLAUDE_CONFIG_DIR",
            "TMP",
            "TEMP",
        ] {
            child.env(key, &directory);
        }
        if scenario != "unset" {
            child.env(
                "PNS_CHANNELS_DIR",
                if scenario == "blank" {
                    "".into()
                } else {
                    directory.as_os_str().to_owned()
                },
            );
        }
        let mut child = child.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_millis(500);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn check_factory(scenario: &str) {
    let directory = PathBuf::from(std::env::var_os("HOME").unwrap());
    let mut declarations = Registry::new();
    declarations.register_channel("mobile", ROUTING).unwrap();
    let mobile = Mobile {
        refusal: (scenario == "refused").then(|| "unknown backend".into()),
        ..Mobile::default()
    };
    let selected = destinations(
        &declarations.all(),
        "priority",
        directory.to_str().unwrap(),
        &mobile,
        None,
    );
    let outcome = selected.deliver("mobile", &request(&Event::default()));
    match scenario {
        "forced" => {
            assert_eq!(outcome, Delivery::Silent);
            assert_eq!(
                std::fs::read_to_string(directory.join("id")).unwrap(),
                "original-92"
            );
            assert_eq!(
                std::fs::read_to_string(directory.join("producer")).unwrap(),
                "fixture"
            );
            assert_eq!(
                std::fs::read(directory.join("body")).unwrap(),
                format!(
                    "{}\n",
                    pns_adapters::event_json(&Event::default(), ReportMode::Silent)
                )
                .as_bytes()
            );
        }
        "refused" => {
            assert_eq!(
                outcome,
                Delivery::Failed(refused_backend_line("unknown backend"))
            );
            assert!(!directory.join("body").exists());
        }
        "unset" | "blank" => {
            assert!(
                matches!(outcome, Delivery::Failed(line) if line.contains("[plugins.mobile] token"))
            );
            assert!(!directory.join("body").exists());
        }
        _ => panic!("unknown private scenario"),
    }
}
