use super::*;

fn parsed(text: &str) -> DeliveryPath {
    Delivery::parse(text).expect("a usable delivery choice")
}

#[test]
fn producer_mode_carries_the_command_and_its_arguments_verbatim() {
    let path = parsed(
        r#"
        [delivery]
        mode = "producer"
        [delivery.producer]
        command = "/private/fixture/engine"
        arguments = ["submit", "--json"]
        "#,
    );
    assert_eq!(
        path,
        DeliveryPath::Producer {
            command: PathBuf::from("/private/fixture/engine"),
            arguments: vec!["submit".to_string(), "--json".to_string()],
        }
    );
}

#[test]
fn hermes_mode_carries_the_gateway_base_and_one_key_per_route() {
    let path = parsed(
        r#"
        [delivery]
        mode = "hermes"
        [delivery.hermes]
        url = "http://127.0.0.1:8644/webhooks"
        [delivery.hermes.keys]
        posture-pages = "k-posture-pages"
        priority = "k-priority"
        "#,
    );
    let DeliveryPath::Hermes { base_url, keys } = path else {
        panic!("hermes mode")
    };
    assert_eq!(base_url, "http://127.0.0.1:8644/webhooks");
    assert_eq!(
        keys.get("posture-pages").map(String::as_str),
        Some("k-posture-pages")
    );
    assert_eq!(keys.get("priority").map(String::as_str), Some("k-priority"));
}

#[test]
fn a_mode_this_build_does_not_serve_is_refused_by_name() {
    let refusal = Delivery::parse("[delivery]\nmode = \"banner\"\n").unwrap_err();
    assert!(refusal.contains("banner"), "{refusal}");
    assert!(refusal.contains("producer"), "{refusal}");
    assert!(refusal.contains("hermes"), "{refusal}");
}

#[test]
fn a_misspelled_key_or_table_blocks_the_file_rather_than_switching_delivery_off() {
    for text in [
        "[delivery]\nmode = \"hermes\"\nurl = \"http://x\"\n",
        "[delivery]\nmode = \"producer\"\n[delivery.producer]\ncommand = \"/x\"\narguents = []\n",
        "[delivary]\nmode = \"hermes\"\n",
    ] {
        assert!(Delivery::parse(text).is_err(), "{text}");
    }
}

#[test]
fn producer_mode_without_a_command_is_refused_rather_than_left_unrunnable() {
    let refusal = Delivery::parse("[delivery]\nmode = \"producer\"\n").unwrap_err();
    assert!(refusal.contains("command"), "{refusal}");
}

#[test]
fn hermes_mode_states_the_local_gateway_when_the_file_names_none() {
    let path = parsed("[delivery]\nmode = \"hermes\"\n");
    assert_eq!(
        path,
        DeliveryPath::Hermes {
            base_url: DEFAULT_WEBHOOK_BASE.to_string(),
            keys: BTreeMap::new(),
        }
    );
}

#[test]
fn an_absent_file_leaves_the_fail_closed_default_and_no_refusal_to_report() {
    let delivery = Delivery::read(Path::new("/private/fixture/absent"));
    assert_eq!(delivery, Delivery::default());
    assert_eq!(delivery.refusal, None);
    assert_eq!(
        delivery.path,
        DeliveryPath::Hermes {
            base_url: DEFAULT_WEBHOOK_BASE.to_string(),
            keys: BTreeMap::new(),
        }
    );
}

#[test]
fn a_malformed_file_names_its_own_refusal_and_never_reads_as_an_absent_one() {
    let sandbox = crate::test_sandbox::Sandbox::new("delivery-malformed");
    let home = sandbox.path();
    std::fs::create_dir_all(home.join(".config/posture")).unwrap();
    std::fs::write(config_path(home), "[delivery\nmode = \"hermes\"\n").unwrap();
    let delivery = Delivery::read(home);
    assert!(delivery.refusal.is_some());
    assert_eq!(delivery.path, Delivery::default().path);
}

#[test]
fn formatting_a_delivery_names_the_routes_and_never_prints_a_signing_key() {
    let delivery = Delivery {
        path: parsed(
            r#"
            [delivery]
            mode = "hermes"
            [delivery.hermes.keys]
            posture-pages = "s3cret-posture"
            priority = "s3cret-priority"
            "#,
        ),
        refusal: None,
    };
    let formatted = format!("{delivery:?}");
    assert!(!formatted.contains("s3cret"), "{formatted}");
    assert!(formatted.contains("posture-pages"), "{formatted}");
    assert!(formatted.contains("priority"), "{formatted}");
}

#[test]
fn a_config_path_left_by_a_broken_link_is_refused_rather_than_read_as_unconfigured() {
    let sandbox = crate::test_sandbox::Sandbox::new("delivery-dangling-link");
    let home = sandbox.path();
    std::fs::create_dir_all(home.join(".config/posture")).unwrap();
    std::os::unix::fs::symlink(home.join("nowhere"), config_path(home)).unwrap();
    let delivery = Delivery::read(home);
    assert!(delivery.refusal.is_some(), "{delivery:?}");
    assert_eq!(delivery.path, Delivery::default().path);
}
