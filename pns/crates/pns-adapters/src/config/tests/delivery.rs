use super::*;

#[test]
fn delivery_classes_default_explicitly_and_only_valid_configured_names_can_bypass() {
    let configured = parse_config("[delivery]\nbypass_silence_classes = [\"custom\"]")
        .expect("a delivery policy");
    assert_eq!(configured.bypass_silence_classes, ["custom"]);
    assert_eq!(
        configured.silence_policy(Some("custom")),
        pns_domain::SilencePolicy::BypassBannerAndPhone
    );
    for class in [None, Some("security"), Some("Custom"), Some(" custom")] {
        assert_eq!(
            configured.silence_policy(class),
            pns_domain::SilencePolicy::Respect
        );
    }
    assert_eq!(
        parse_config("").unwrap().bypass_silence_classes,
        ["security"]
    );
    assert_eq!(
        parse_config("[delivery]").unwrap().bypass_silence_classes,
        ["security"]
    );
    assert!(
        parse_config("[delivery]\nbypass_silence_classes = []")
            .unwrap()
            .bypass_silence_classes
            .is_empty()
    );
    for value in ["3", "[3]", "[\"\"]", "[\"bad\\nclass\"]"] {
        assert!(
            parse_config(&format!("[delivery]\nbypass_silence_classes = {value}")).is_err(),
            "{value}"
        );
    }
    for size in [64, 65] {
        let text = format!(
            "[delivery]\nbypass_silence_classes = [\"{}\"]",
            "x".repeat(size)
        );
        assert_eq!(parse_config(&text).is_ok(), size == 64, "{size}");
    }
    assert!(refusal("[delivery]\nbypass_classes = []").contains("bypass_classes"));
    assert!(refusal("delivery = false").contains("delivery"));
}
