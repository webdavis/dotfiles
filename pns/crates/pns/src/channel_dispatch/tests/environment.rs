use super::*;

/// THE DECISION `destinations_for_override` MAKES, driven by the override
/// value rather than by the process environment: an absent or blank
/// `PNS_CHANNELS_DIR` leaves the native backend in place, a set one forces the
/// executable, and a refused backend precedes either. The `PNS_CHANNELS_DIR`
/// read itself is the one line above that function.
///
/// THIS USED TO RE-EXEC THE TEST BINARY once per scenario with a scrubbed
/// environment, bounded by a 500ms wall-clock deadline, and it reddened `main`
/// on untouched code: a spawn that outran the budget was killed, and the kill
/// read as the factory failing. Nothing here waits on a clock now.
#[test]
fn a_blank_channels_override_falls_through_and_a_refused_backend_precedes_dispatch() {
    for scenario in ["unset", "blank", "forced", "refused"] {
        let directory = fixture("mobile");
        let override_dir = match scenario {
            "unset" => None,
            "blank" => Some(""),
            _ => Some(directory.to_str().unwrap()),
        };
        let mut declarations = Registry::new();
        declarations.register_channel("mobile", ROUTING).unwrap();
        let mobile = Mobile {
            refusal: (scenario == "refused").then(|| "unknown backend".into()),
            ..Mobile::default()
        };
        let selected = destinations_for_override(
            override_dir,
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
            // A BLANK OVERRIDE IS NOT A DIRECTORY: both of these keep the
            // native moshi backend, which names the config key to write
            // instead of launching anything.
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
