use super::*;

/// THE DECISION `destinations_for_override` MAKES, driven by the settings
/// handed in rather than by the process environment: an unnamed channels
/// directory leaves the native backend in place, a named one forces the
/// executable, and a refused backend precedes either. Where the name comes
/// from, and that a blank one names nothing, is `install_settings`' own test.
///
/// THIS USED TO RE-EXEC THE TEST BINARY once per scenario with a scrubbed
/// environment, bounded by a 500ms wall-clock deadline, and it reddened `main`
/// on untouched code: a spawn that outran the budget was killed, and the kill
/// read as the factory failing. Nothing here waits on a clock now.
#[test]
fn an_unnamed_channels_directory_falls_through_and_a_refused_backend_precedes_dispatch() {
    for scenario in ["unset", "forced", "refused"] {
        let directory = fixture("mobile");
        let install = InstallSettings {
            state_dir: None,
            channels_dir: (scenario != "unset").then(|| directory.to_str().unwrap().to_string()),
            hermes_url: None,
            moshi_url: None,
            terminal_bundle_id: None,
            remote_deadline: None,
            busy_deadline: pns_adapters::DEFAULT_BUSY_DEADLINE,
        };
        let mut declarations = Registry::new();
        declarations.register_channel("mobile", ROUTING).unwrap();
        let mobile = Mobile {
            refusal: (scenario == "refused").then(|| "unknown backend".into()),
            ..Mobile::default()
        };
        let selected = destinations_for_override(
            &install,
            &declarations.all(),
            "priority",
            directory.to_str().unwrap(),
            &mobile,
            &pns_adapters::HermesKeys::default(),
            &pns_adapters::DiscordSettings::default(),
            &pns_domain::routes::Routes::default(),
            false,
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
            // NO DIRECTORY NAMED keeps the native moshi backend, which names
            // the config key to write instead of launching anything.
            _ => {
                assert!(
                    matches!(&outcome, Delivery::Failed(line) if line.contains("[plugins.mobile] token")),
                    "{scenario}: {outcome:?}"
                );
                assert!(!directory.join("body").exists());
            }
        }
    }
}
