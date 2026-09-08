use super::*;

#[test]
fn setup_keeps_every_credential_hidden_and_delivers_all_other_answers_to_the_renderer() {
    let world = World::new(&[
        "phone secret",
        "yes",
        "hermes secret",
        "yes",
        "bridge",
        "hue secret",
        "Studio, Kitchen",
        "yes",
        "UniFi",
        "router url",
        "router secret",
        "phone",
        "yes",
        "Sleep, Work",
        "yes",
    ]);
    assert_eq!(world.run(false).0, 0);
    let questions: Vec<String> = world
        .trace
        .borrow()
        .iter()
        .filter(|line| line.starts_with("secret:"))
        .cloned()
        .collect();
    assert_eq!(questions.len(), 4);
    assert!(questions[0].contains("webhook secret"));
    assert!(questions[1].contains("signing key"));
    assert!(questions[2].contains("bridge issued"));
    assert!(questions[3].contains("router issued"));
    assert_eq!(
        *world.observed.borrow(),
        Some(Answers {
            mobile_token: "phone secret".into(),
            hermes_key: "hermes secret".into(),
            hue_bridge: "bridge".into(),
            hue_key: "hue secret".into(),
            hue_rooms: vec!["Studio".into(), "Kitchen".into()],
            router_type: "unifi".into(),
            router_url: "router url".into(),
            router_api_key: "router secret".into(),
            router_device_hostname: "phone".into(),
            focus_modes: vec!["Sleep".into(), "Work".into()],
            nag: true
        })
    );
    for credential in [
        "phone secret",
        "hermes secret",
        "hue secret",
        "router secret",
    ] {
        assert!(!world.output.borrow().join("\n").contains(credential));
    }
    assert!(world.answers.borrow().is_empty());
}

#[test]
fn setup_blank_hue_address_and_unknown_router_decline_without_unused_credentials() {
    let world = World::new(&["", "no", "yes", "", "yes", "not-a-router", "no", "no"]);
    assert_eq!(world.run(false).0, 0);
    assert!(world.answers.borrow().is_empty());
    assert_eq!(
        world
            .trace
            .borrow()
            .iter()
            .filter(|line| line.starts_with("secret:"))
            .count(),
        1
    );
    let output = world.output.borrow().join("\n");
    assert!(output.contains("nothing given, so the light pulse stays off"));
    assert!(output.contains("nothing here reads that router, so the home probe stays off"));
    assert_eq!(*world.observed.borrow(), Some(Answers::default()));
}
