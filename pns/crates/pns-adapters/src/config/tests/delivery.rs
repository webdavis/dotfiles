use super::*;

#[test]
fn a_class_table_names_its_route_and_whether_it_passes_the_mute() {
    let configured = parse_config(
        "[delivery_class.security]\nroute = \"pages\"\nbypass_mute = true\n\
         [delivery_class.quiet-one]\n",
    )
    .expect("a delivery-class policy");
    let security = configured.delivery_class("security").expect("security");
    assert_eq!(security.route, "pages");
    assert!(security.bypass_mute);
    // BOTH KEYS DEFAULT, so a table writing neither is a complete statement:
    // the routine route, and the mute respected.
    let quiet = configured.delivery_class("quiet-one").expect("quiet-one");
    assert_eq!(quiet.route, "");
    assert!(!quiet.bypass_mute);
    assert_eq!(
        configured.silence_policy("security"),
        pns_domain::SilencePolicy::BypassBannerAndPhone
    );
    assert_eq!(
        configured.silence_policy("quiet-one"),
        pns_domain::SilencePolicy::Respect
    );
}

#[test]
fn a_message_naming_no_class_reads_the_default_table_and_a_class_no_table_defines_is_refused() {
    let configured = parse_config(
        "[delivery_class.default]\nroute = \"logbook\"\nbypass_mute = true\n\
         [delivery_class.health]\nroute = \"sirens\"\n",
    )
    .expect("a delivery-class policy");
    let unnamed = configured.delivery_class("").expect("the default table");
    assert_eq!(unnamed.route, "logbook");
    assert!(unnamed.bypass_mute);
    assert_eq!(
        configured.silence_policy(""),
        pns_domain::SilencePolicy::BypassBannerAndPhone
    );
    // NAMING NO CLASS IS NEVER A REFUSAL, however the file is written: it is
    // what every harness hook and the shell notifier send.
    assert!(!configured.refuses_delivery_class(""));
    assert!(!parse_config("").unwrap().refuses_delivery_class(""));

    // A NEAR MISS IS THE WHOLE POINT. `Health` and ` health` are words the
    // operator never defined, and a page delivered on either would land on a
    // route they did not choose.
    for class in ["security", "Health", " health", "agent"] {
        assert!(configured.refuses_delivery_class(class), "{class}");
        assert!(configured.delivery_class(class).is_none(), "{class}");
    }
    assert!(!configured.refuses_delivery_class("health"));
}

#[test]
fn a_file_defining_no_class_defines_none_rather_than_a_shipped_set() {
    // pns COMPILES IN NO CLASS WORDS: an empty file leaves every class
    // undefined, which is what makes the shipped config the one statement of
    // which classes this machine serves.
    for text in ["", "[delivery]", "[delivery]\nmax_retries = 3"] {
        let config = parse_config(text).unwrap_or_else(|error| panic!("{text}: {error:?}"));
        assert!(config.delivery_classes.is_empty(), "{text}");
        assert_eq!(
            config.silence_policy("security"),
            pns_domain::SilencePolicy::Respect,
            "{text}"
        );
    }
}

#[test]
fn every_unusable_class_name_key_and_value_is_refused_by_name() {
    for (text, named) in [
        ("[delivery_class.security]\nbypass_mute = 3", "bypass_mute"),
        ("[delivery_class.security]\nroute = 3", "route"),
        ("[delivery_class.security]\nroute = \"a b\"", "route"),
        ("[delivery_class.security]\nbypass = true", "bypass"),
        ("delivery_class = false", "delivery_class"),
        ("[delivery_class]\nsecurity = 3", "security"),
    ] {
        assert!(refusal(text).contains(named), "{text}");
    }
    for size in [64, 65] {
        let text = format!("[delivery_class.\"{}\"]\n", "x".repeat(size));
        assert_eq!(parse_config(&text).is_ok(), size == 64, "{size}");
    }
}

#[test]
fn the_retired_bypass_list_is_refused_by_name_rather_than_ignored() {
    // AN OPERATOR CARRYING THE OLD KEY IS TOLD, because a bypass they believe
    // is in force and is not is a page held back by a mute they set.
    let refused = refusal("[delivery]\nbypass_silence_classes = [\"security\"]");
    assert!(refused.contains("bypass_silence_classes"), "{refused}");
    assert!(refused.contains("max_retries"), "{refused}");
    assert!(refusal("delivery = false").contains("delivery"));
}
